use std::{
    any::{Any, TypeId},
    cell::RefCell,
    collections::HashMap,
    fs,
    marker::PhantomData,
    process::Command,
    rc::Rc,
};

use anyhow::bail;
use url::Url;

use crate::config::{Config, LocalConf};

use super::{BeamProxy, BrokerProvider, ForwardProxy, Service};

#[derive(Debug)]
pub struct OidcClient<T: OidcProvider> {
    beam_proxy: BeamProxy<T::BeamProvider>,
    site_id: String,
    http_proxy_url: Option<Url>,
    pub_redirect_paths: Vec<String>,
    priv_redirect_urls: Vec<String>,
    local_conf: Rc<RefCell<LocalConf>>,
    synced: bool,
}

impl<T: OidcProvider> OidcClient<T> {
    fn new(conf: &Config) -> Self {
        let mut dummy_fw_proxy = ForwardProxy::from_config(conf, ());
        let beam_proxy = BeamProxy::from_config(conf, (&mut dummy_fw_proxy,));
        let proxy_url = dummy_fw_proxy.https_proxy_url;
        Self {
            site_id: conf.site_id.clone(),
            beam_proxy,
            pub_redirect_paths: Default::default(),
            priv_redirect_urls: Default::default(),
            http_proxy_url: proxy_url,
            local_conf: conf.local_conf.clone(),
            synced: false,
        }
    }
}

thread_local! {
    static OIDC_CLIENTS: RefCell<HashMap<TypeId, Box<dyn Any>>> = Default::default();
}

impl<T: OidcProvider> OidcClient<T> {
    pub fn add_public_redirect_path(conf: &Config, path: &str) -> PublicOidcClient<T> {
        OIDC_CLIENTS.with_borrow_mut(|m| {
            m.entry(TypeId::of::<T>())
                .or_insert_with(|| Box::new(Self::new(conf)))
                .downcast_mut::<Self>()
                .unwrap()
                .pub_redirect_paths
                // TODO: Host handeling
                .push(path.to_owned());
        });
        PublicOidcClient {
            provider: PhantomData,
            client_id: format!("{}-public", conf.site_id),
        }
    }

    pub fn add_private_redirect_path(conf: &Config, path: &str) -> PrivateOidcClient<T> {
        OIDC_CLIENTS.with_borrow_mut(|m| {
            m.entry(TypeId::of::<T>())
                .or_insert_with(|| Box::new(Self::new(conf)))
                .downcast_mut::<Self>()
                .unwrap()
                .priv_redirect_urls
                // TODO: Host handeling
                .push(path.to_owned());
        });
        PrivateOidcClient {
            provider: PhantomData,
            client_id: format!("{}-private", conf.site_id),
        }
    }

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
        let root_cert_file =
            std::env::temp_dir().join(format!("{}.pem", T::BeamProvider::network_name()));
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
        fs::create_dir_all("/usr/local")?;
        let cached_data = self
            .local_conf
            .borrow()
            .oidc
            .iter()
            .flatten()
            .map(|(k, v)| format!("{k}=\"{v}\""))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write("/usr/local/cache", cached_data)?;
        let mut secret_sync = Command::new("local")
            .env("PROXY_ID", &self.beam_proxy.proxy_id)
            .env("OIDC_PROVIDER", T::oidc_provider_id())
            .env("SECRET_DEFINITIONS", secret_sync_defs.join("\x1E"))
            .spawn()?;
        secret_sync.wait()?;
        beam_proxy.kill()?;
        let out = fs::read_to_string("/usr/local/cache")?;
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

    fn evaluate() -> Rc<RefCell<LocalConf>> {
        OIDC_CLIENTS.with_borrow_mut(|m| {
            let client_spec = m
                .get_mut(&TypeId::of::<T>())
                .unwrap()
                .downcast_mut::<Self>()
                .unwrap();
            if let Err(e) = client_spec.sync() {
                eprintln!(
                    "Failed to sync oidc client via {}: {e:#}",
                    T::oidc_provider_id()
                );
            }
            client_spec.local_conf.clone()
        })
    }
}

pub struct PublicOidcClient<T: OidcProvider> {
    provider: PhantomData<T>,
    client_id: String,
}

impl<T: OidcProvider> PublicOidcClient<T> {
    pub fn client_id(&self) -> &str {
        OidcClient::<T>::evaluate();
        &self.client_id
    }
}

pub struct PrivateOidcClient<T: OidcProvider> {
    provider: PhantomData<T>,
    client_id: String,
}

impl<T: OidcProvider> PrivateOidcClient<T> {
    pub fn client_id(&self) -> &str {
        OidcClient::<T>::evaluate();
        &self.client_id
    }

    pub fn client_secret(&self) -> String {
        let private_client_name = format!("{}_client_secret", T::BeamProvider::network_name());
        OidcClient::<T>::evaluate()
            .borrow()
            .oidc
            .as_ref()
            .unwrap()
            .get(&private_client_name)
            .unwrap()
            .clone()
    }
}

pub trait OidcProvider: 'static {
    type BeamProvider: BrokerProvider;

    fn oidc_provider_id() -> String;

    fn issuer_url() -> Url;
}
