use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs,
    marker::PhantomData,
    path::PathBuf,
    str::FromStr,
};

use askama::Template;
use url::Url;

use crate::{Config, config::LocalConf, utils::filters};

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
    app_keys: BTreeMap<&'static str, String>,
    fw_proxy_url: Url,
    local_conf: &'static RefCell<LocalConf>,
}

#[derive(Debug)]
pub struct BeamAppInfos {
    pub id: String,
    pub secret: String,
    pub url: Url,
}

impl<T: BrokerProvider> BeamProxy<T> {
    pub fn add_service(&mut self, service_name: &'static str) -> BeamAppInfos {
        let secret_var = self.app_keys.entry(service_name).or_insert_with(|| {
            self.local_conf
                .borrow_mut()
                .generate_secret::<10, Self>(format!("{service_name}_KEY").as_str())
        });
        BeamAppInfos {
            id: format!("{service_name}.{}", self.proxy_id),
            secret: secret_var.clone(),
            url: Url::from_str(&format!("http://{}:8081", Self::service_name())).unwrap(),
        }
    }
}

impl<T: BrokerProvider> Service for BeamProxy<T> {
    type Dependencies = (ForwardProxy,);
    type ServiceConfig = &'static Config;

    fn from_config(conf: Self::ServiceConfig, (fw_proxy,): Deps<Self>) -> Self {
        BEAM_NETWORKS.with_borrow_mut(|nets| nets.insert(T::broker_id()));
        fs::create_dir_all(conf.path.join("pki")).unwrap();
        BeamProxy {
            broker_provider: PhantomData,
            priv_key: conf.path.join(format!("pki/{}.priv.pem", conf.site_id)),
            proxy_id: format!("{}.{}", conf.site_id, T::broker_id()),
            app_keys: Default::default(),
            fw_proxy_url: fw_proxy.get_url(),
            trusted_ca_certs: conf.trusted_ca_certs(),
            local_conf: &conf.local_conf,
        }
    }

    fn service_name() -> String {
        format!("{}-beam-proxy", T::network_name())
    }
}

thread_local! {
    pub static BEAM_NETWORKS: RefCell<BTreeSet<String>> = RefCell::default();
}
