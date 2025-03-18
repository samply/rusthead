use std::{collections::HashMap, marker::PhantomData, path::PathBuf, str::FromStr};

use rinja::Template;
use url::Url;

use crate::{utils::generate_password, Config};

use super::Service;

pub trait BrokerProvider: 'static {
    fn broker_url() -> Url;
    fn network_name() -> &'static str;
    fn root_cert() -> &'static str;
}

#[derive(Debug, Template)]
#[template(path = "beam.yml")]
pub struct BeamProxy<T: BrokerProvider> {
    broker_provider: PhantomData<T>,
    proxy_id: String,
    priv_key: PathBuf,
    app_keys: HashMap<&'static str, String>,
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
        Url::from_str(&format!("http://{}", Self::service_name())).unwrap()
    }
}

impl<T: BrokerProvider> Service for BeamProxy<T> {
    type Dependencies<'a> = ();

    fn from_config(conf: &Config, _: Self::Dependencies<'_>) -> Self {
        BeamProxy {
            broker_provider: PhantomData,
            priv_key: conf.path.join(format!("pki/{}.priv.pem", conf.site_id)),
            proxy_id: format!("{}.{}", conf.site_id, T::broker_url().host().unwrap()),
            app_keys: Default::default(),
        }
    }

    fn service_name() -> String {
        format!("{}-beam-proxy", T::network_name())
    }
}
