use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    marker::PhantomData,
    path::PathBuf,
    str::FromStr,
};

use askama::Template;
use url::Url;

use crate::{
    utils::{filters, generate_password},
    Config,
};

use super::{Deps, ForwardProxy, Service};

pub trait BrokerProvider: 'static {
    fn broker_url() -> Url;
    fn network_name() -> &'static str;
    fn root_cert() -> &'static str;

    fn broker_id() -> String {
        Self::broker_url().host().unwrap().to_string()
    }
}

#[derive(Debug, Template)]
#[template(path = "beam.yml")]
pub struct BeamProxy<T: BrokerProvider> {
    broker_provider: PhantomData<T>,
    pub proxy_id: String,
    pub priv_key: PathBuf,
    pub trusted_ca_certs: PathBuf,
    app_keys: HashMap<&'static str, String>,
    fw_proxy_url: Url,
}

impl<T: BrokerProvider> BeamProxy<T> {
    /// Returns (BeamAppId, BeamSecret)
    pub fn add_service(&mut self, service_name: &'static str) -> (String, String) {
        let secret = self
            .app_keys
            .entry(service_name)
            .or_insert_with(generate_password::<16>);
        (format!("{service_name}.{}", self.proxy_id), secret.clone())
    }

    pub fn get_url(&self) -> Url {
        Url::from_str(&format!("http://{}:8081", Self::service_name())).unwrap()
    }
}

impl<T: BrokerProvider> Service for BeamProxy<T> {
    type Dependencies<'a> = (ForwardProxy,);

    fn from_config(conf: &Config, (fw_proxy,): Deps<'_, Self>) -> Self {
        BEAM_NETWORKS.with_borrow_mut(|nets| nets.insert(T::broker_id()));
        BeamProxy {
            broker_provider: PhantomData,
            priv_key: conf.path.join(format!("pki/{}.priv.pem", conf.site_id)),
            proxy_id: format!("{}.{}", conf.site_id, T::broker_id()),
            app_keys: Default::default(),
            fw_proxy_url: fw_proxy.get_url(),
            trusted_ca_certs: conf.trusted_ca_certs(),
        }
    }

    fn service_name() -> String {
        format!("{}-beam-proxy", T::network_name())
    }
}

thread_local! {
    pub static BEAM_NETWORKS: RefCell<HashSet<String>> = RefCell::default();
}
