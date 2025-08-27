use std::{
    any::{Any, TypeId},
    cell::RefCell,
    collections::HashMap,
    fs,
    process::Command,
};

use anyhow::bail;
use url::{Host, Url};

use crate::config::{Config, LocalConf};

use super::{BeamProxy, BrokerProvider, ForwardProxy, Service};

#[derive(Debug)]
pub struct OidcClient<T: OidcProvider> {
    beam_proxy: BeamProxy<T::BeamProvider>,
    site_id: String,
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
            site_id: conf.site_id.clone(),
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
        if secret_sync_defs.is_empty() {
            bail!("No secrets to sync")
        }
        let temp_dir = std::env::temp_dir();
        let root_cert_file = temp_dir.join(format!("{}.pem", T::BeamProvider::network_name()));
        fs::write(&root_cert_file, T::BeamProvider::root_cert())?;
        let mut beam_proxy_conf = Command::new("proxy");
        beam_proxy_conf
            .env("RUST_LOG", "warn")
            .env("PRIVKEY_FILE", &self.beam_proxy.priv_key)
            .env("ROOTCERT_FILE", root_cert_file)
            .env("BROKER_URL", T::BeamProvider::broker_url().as_str())
            .env("PROXY_ID", &self.beam_proxy.proxy_id)
            .env("TLS_CA_CERTIFICATES_DIR", &self.beam_proxy.trusted_ca_certs)
            .env("APP_secret-sync_KEY", "NotSecret");
        if let Some(http_proxy) = &self.http_proxy_url {
            beam_proxy_conf.env("ALL_PROXY", http_proxy.as_str());
        }
        let mut beam_proxy = beam_proxy_conf.spawn()?;
        let cached_data = self
            .local_conf
            .borrow()
            .oidc
            .iter()
            .flatten()
            .map(|(k, v)| format!("{k}=\"{v}\""))
            .collect::<Vec<_>>()
            .join("\n");
        let cache_path = temp_dir.join("cache");
        fs::write(&cache_path, cached_data)?;
        let mut secret_sync = Command::new("local")
            .env("PROXY_ID", &self.beam_proxy.proxy_id)
            .env("OIDC_PROVIDER", T::oidc_provider_id())
            .env("SECRET_DEFINITIONS", secret_sync_defs.join("\x1E"))
            .env("CACHE_PATH", &cache_path)
            .spawn()?;
        secret_sync.wait()?;
        beam_proxy.kill()?;
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
        Host::Ipv4(ipv4_addr) => out.push(dbg!(format!("https://{ipv4_addr}{path}"))),
        Host::Ipv6(ipv6_addr) => out.push(format!("https://[{ipv6_addr}]{path}")),
    }
    out
}

pub struct PublicOidcClient {
    provider: TypeId,
    client_id: String,
}

impl PublicOidcClient {
    pub fn client_id(&self) -> &str {
        evaluate(self.provider);
        &self.client_id
    }
}

pub struct PrivateOidcClient {
    provider: TypeId,
    client_id: String,
    private_client_name: String,
}

impl PrivateOidcClient {
    pub fn client_id(&self) -> &str {
        evaluate(self.provider);
        &self.client_id
    }

    pub fn client_secret(&self) -> String {
        evaluate(self.provider)
            .borrow()
            .oidc
            .as_ref()
            .unwrap()
            .get(&self.private_client_name)
            .cloned()
            // HACK: If we have a config that requires oidc and we are not enrolled yet we don't want to panic
            // as that will prevent generation of the bridgehead script so lets default to an empty string.
            .unwrap_or_default()
    }
}

pub trait OidcProvider: 'static {
    type BeamProvider: BrokerProvider;

    fn oidc_provider_id() -> String;

    fn issuer_url() -> Url;
}
