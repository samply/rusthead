use std::{
    any::{Any, TypeId},
    cell::RefCell,
    collections::HashMap,
    fs,
};

use anyhow::{Context, bail};
use url::{Host, Url};

use crate::config::{Config, LocalConf};

use super::{BeamProxy, BrokerProvider, ForwardProxy, Service};

#[derive(Debug)]
pub struct OidcClient<T: OidcProvider> {
    beam_proxy: BeamProxy<T::BeamProvider>,
    http_proxy_url: Option<Url>,
    pub_redirect_paths: Vec<String>,
    priv_redirect_urls: Vec<String>,
    local_conf: &'static RefCell<LocalConf>,
    synced: bool,
}

thread_local! {
    static OIDC_CLIENTS: RefCell<HashMap<TypeId, Box<dyn SyncOidc>>> = Default::default();
}

trait SyncOidc: Any {
    fn sync(&mut self) -> anyhow::Result<()>;

    fn get_local_conf(&self) -> &'static RefCell<LocalConf>;
}

impl<T: OidcProvider> OidcClient<T> {
    fn new(conf: &'static Config) -> Self {
        let mut dummy_fw_proxy = ForwardProxy::from_config(conf, ());
        let beam_proxy = BeamProxy::from_config(conf, (&mut dummy_fw_proxy,));
        let proxy_url = dummy_fw_proxy.https_proxy_url;
        Self {
            beam_proxy,
            pub_redirect_paths: Default::default(),
            priv_redirect_urls: Default::default(),
            http_proxy_url: proxy_url,
            local_conf: &conf.local_conf,
            synced: false,
        }
    }

    pub fn add_public_redirect_path(conf: &'static Config, path: &str) -> PublicOidcClient {
        OIDC_CLIENTS.with_borrow_mut(|m| {
            let syncer = m
                .entry(TypeId::of::<T>())
                .or_insert_with(|| Box::new(Self::new(conf)))
                .as_mut() as &mut dyn Any;
            syncer
                .downcast_mut::<Self>()
                .unwrap()
                .pub_redirect_paths
                .extend(redirect_urls_for_path(path, &conf.hostname));
        });
        PublicOidcClient {
            provider: TypeId::of::<T>(),
            client_id: format!("{}-public", conf.site_id),
            get_issuer_url: T::issuer_url,
        }
    }

    pub fn add_private_redirect_path(conf: &'static Config, path: &str) -> PrivateOidcClient {
        OIDC_CLIENTS.with_borrow_mut(|m| {
            let syncer = m
                .entry(TypeId::of::<T>())
                .or_insert_with(|| Box::new(Self::new(conf)))
                .as_mut() as &mut dyn Any;
            syncer
                .downcast_mut::<Self>()
                .unwrap()
                .priv_redirect_urls
                .extend(redirect_urls_for_path(path, &conf.hostname));
        });
        PrivateOidcClient {
            provider: TypeId::of::<T>(),
            client_id: format!("{}-private", conf.site_id),
            private_client_name: format!("{}_client_secret", T::BeamProvider::network_name()),
            get_issuer_url: T::private_issuer_url,
        }
    }
}

impl<T: OidcProvider> SyncOidc for OidcClient<T> {
    fn sync(&mut self) -> anyhow::Result<()> {
        if self.synced {
            return Ok(());
        }
        self.synced = true;
        let mut secret_sync_defs = Vec::new();
        let public_client_name = format!("{}_public_client", T::BeamProvider::network_name());
        if !self.pub_redirect_paths.is_empty() {
            let public_urls = self.pub_redirect_paths.join(",");
            secret_sync_defs.push(format!("OIDC:{public_client_name}:public;{public_urls}"));
        }
        let private_client_name = format!("{}_client_secret", T::BeamProvider::network_name());
        if !self.priv_redirect_urls.is_empty() {
            let priv_urls = self.priv_redirect_urls.join(",");
            secret_sync_defs.push(format!("OIDC:{private_client_name}:private;{priv_urls}"));
        }
        #[cfg(debug_assertions)]
        {
            if let Some(cached) = self.local_conf.borrow().oidc.as_ref()
                && [&public_client_name, &private_client_name]
                    .iter()
                    .all(|&name| cached.contains_key(name))
            {
                return Ok(());
            }
        }
        if secret_sync_defs.is_empty() {
            bail!("No secrets to sync")
        }
        let temp_dir = tempfile::tempdir().context("Failed to create secret-sync directory")?;
        let root_cert_file = temp_dir.path().join("root.crt.pem");
        fs::write(&root_cert_file, T::BeamProvider::root_cert())?;
        let cached_data = self
            .local_conf
            .borrow()
            .oidc
            .iter()
            .flatten()
            .map(|(k, v)| format!("{k}=\"{v}\""))
            .collect::<Vec<_>>()
            .join("\n");
        let cache_path = temp_dir.path().join("cache");
        fs::write(&cache_path, cached_data)?;
        secret_sync_command(
            &cache_path,
            &self.beam_proxy.priv_key,
            &root_cert_file,
            &self.beam_proxy.trusted_ca_certs,
            &self.beam_proxy.proxy_id,
            T::BeamProvider::broker_url().as_str(),
            &T::oidc_provider_id(),
            &secret_sync_defs.join("\x1E"),
            self.http_proxy_url.as_ref().map(Url::as_str),
        )?
        .run()
        .context("Secret-sync container failed")?;
        let out = fs::read_to_string(cache_path)?;
        let new_cache = out
            .lines()
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k, v.trim_matches('"')))
            .collect::<HashMap<_, _>>();
        let mut local_conf = self.local_conf.borrow_mut();
        let new_oidc_mapping = local_conf.oidc.get_or_insert_default();
        if let Some(cached_pub_client) = new_cache.get(public_client_name.as_str()) {
            new_oidc_mapping.insert(public_client_name, cached_pub_client.to_string());
        }
        if let Some(cached_priv_client) = new_cache.get(private_client_name.as_str()) {
            new_oidc_mapping.insert(private_client_name, cached_priv_client.to_string());
        }
        Ok(())
    }

    fn get_local_conf(&self) -> &'static RefCell<LocalConf> {
        self.local_conf
    }
}

// The image starts both the Beam proxy and the local secret-sync client.
#[allow(clippy::too_many_arguments)]
fn secret_sync_command(
    cache: &std::path::Path,
    key: &std::path::Path,
    root_cert: &std::path::Path,
    trusted_certs: &std::path::Path,
    proxy_id: &str,
    broker_url: &str,
    provider: &str,
    definitions: &str,
    http_proxy: Option<&str>,
) -> anyhow::Result<duct::Expression> {
    // Docker owns the container lifecycle, including failure cleanup. The temporary
    // cache is mounted read/write; keys and trust material are always read-only.
    let mut args = vec!["run".to_owned(), "--rm".to_owned()];
    for (source, target, readonly) in [
        (cache, "/usr/local/cache", false),
        (key, "/run/secrets/privkey.pem", true),
        (root_cert, "/run/secrets/root.crt.pem", true),
        (trusted_certs, "/conf/trusted-ca-certs", true),
    ] {
        let source = source
            .canonicalize()
            .with_context(|| format!("Cannot mount {} for secret-sync", source.display()))?;
        args.extend([
            "-v".to_owned(),
            format!(
                "{}:{target}{}",
                source.display(),
                if readonly { ":ro" } else { "" }
            ),
        ]);
    }
    let mut command = duct::cmd("docker", {
        for name in [
            "TLS_CA_CERTIFICATES_DIR",
            "NO_PROXY",
            "ALL_PROXY",
            "PROXY_ID",
            "BROKER_URL",
            "OIDC_PROVIDER",
            "SECRET_DEFINITIONS",
            "CACHE_PATH",
        ] {
            args.extend(["-e".to_owned(), name.to_owned()]);
        }
        args.push("docker.verbis.dkfz.de/cache/samply/secret-sync-local:latest".to_owned());
        args
    });
    for (name, value) in [
        ("TLS_CA_CERTIFICATES_DIR", "/conf/trusted-ca-certs"),
        ("NO_PROXY", "localhost,127.0.0.1"),
        ("ALL_PROXY", http_proxy.unwrap_or("")),
        ("PROXY_ID", proxy_id),
        ("BROKER_URL", broker_url),
        ("OIDC_PROVIDER", provider),
        ("SECRET_DEFINITIONS", definitions),
        ("CACHE_PATH", "/usr/local/cache"),
    ] {
        command = command.env(name, value);
    }
    Ok(command)
}

fn evaluate(provider: TypeId) -> &'static RefCell<LocalConf> {
    OIDC_CLIENTS.with_borrow_mut(|m| {
        let client_spec = m.get_mut(&provider).unwrap();
        if let Err(e) = client_spec.sync() {
            eprintln!("Failed to sync oidc client: {e:#}");
        }
        client_spec.get_local_conf()
    })
}

fn redirect_urls_for_path(path: &str, host: &Host) -> Vec<String> {
    let mut out = Vec::new();
    match host {
        Host::Domain(domain) => {
            if let Some(without_proxy) = domain.split_once('.').map(|(root_domain, _)| root_domain)
            {
                out.push(format!("https://{without_proxy}{path}"));
            }
            out.push(format!("https://{domain}{path}"));
        }
        Host::Ipv4(ipv4_addr) => out.push(format!("https://{ipv4_addr}{path}")),
        Host::Ipv6(ipv6_addr) => out.push(format!("https://[{ipv6_addr}]{path}")),
    }
    out
}

pub struct PublicOidcClient {
    provider: TypeId,
    client_id: String,
    get_issuer_url: fn(&str) -> Url,
}

impl PublicOidcClient {
    pub fn client_id(&self) -> &str {
        evaluate(self.provider);
        &self.client_id
    }

    pub fn pub_issuer_url(&self) -> Url {
        (self.get_issuer_url)(&self.client_id)
    }
}

#[derive(Debug)]
pub struct PrivateOidcClient {
    provider: TypeId,
    client_id: String,
    private_client_name: String,
    get_issuer_url: fn(&str) -> Url,
}

impl PrivateOidcClient {
    pub fn client_id(&self) -> &str {
        evaluate(self.provider);
        &self.client_id
    }

    pub fn client_secret_var(&self) -> String {
        evaluate(self.provider);
        format!("${{OIDC_{}}}", self.private_client_name.to_uppercase())
    }

    pub fn private_issuer_url(&self) -> Url {
        (self.get_issuer_url)(&self.client_id)
    }
}

pub trait OidcProvider: 'static {
    type BeamProvider: BrokerProvider;

    fn oidc_provider_id() -> String;

    fn issuer_url(_public_client_id: &str) -> Url;

    fn private_issuer_url(_private_client_id: &str) -> Url;

    fn admin_group(conf: &Config) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn secret_sync_runs_the_container_with_isolated_cache_and_readonly_keys() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        for name in ["cache", "key", "root-cert"] {
            fs::write(root.join(name), "existing").unwrap();
        }
        fs::create_dir(root.join("trusted")).unwrap();
        fs::write(root.join("docker"), r#"#!/bin/sh
set -eu
printf '%s\n' "$@" > "$TEST_ROOT/args"
printf '%s\n' "$PROXY_ID" "$BROKER_URL" "$OIDC_PROVIDER" "$SECRET_DEFINITIONS" "$ALL_PROXY" "$NO_PROXY" "$CACHE_PATH" > "$TEST_ROOT/env"
printf 'client="new-secret"\n' > "$TEST_ROOT/cache"
exit "${TEST_EXIT:-0}"
"#).unwrap();
        fs::set_permissions(root.join("docker"), fs::Permissions::from_mode(0o755)).unwrap();
        let command = secret_sync_command(
            &root.join("cache"),
            &root.join("key"),
            &root.join("root-cert"),
            &root.join("trusted"),
            "site.broker",
            "https://broker",
            "provider",
            "OIDC:client:private;https://site",
            Some("http://proxy"),
        )
        .unwrap()
        .env(
            "PATH",
            format!("{}:{}", root.display(), std::env::var("PATH").unwrap()),
        )
        .env("TEST_ROOT", root);
        command.run().unwrap();
        let args = fs::read_to_string(root.join("args")).unwrap();
        assert!(args.starts_with("run\n--rm\n"));
        assert!(args.contains("/run/secrets/privkey.pem:ro"));
        assert!(args.contains("/run/secrets/root.crt.pem:ro"));
        assert!(args.contains("/conf/trusted-ca-certs:ro"));
        assert!(args.contains("/usr/local/cache\n"));
        assert!(args.contains("secret-sync-local:latest"));
        assert!(
            fs::read_to_string(root.join("env"))
                .unwrap()
                .contains("http://proxy\nlocalhost,127.0.0.1\n/usr/local/cache")
        );
        assert!(
            fs::read_to_string(root.join("cache"))
                .unwrap()
                .contains("new-secret")
        );
        assert!(command.env("TEST_EXIT", "9").run().is_err());
    }
}
