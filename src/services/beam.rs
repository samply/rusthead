use std::{collections::HashMap, marker::PhantomData, str::FromStr};

use url::Url;
use rinja::Template;

use crate::{utils::generate_password, Config};

use super::Service;

pub trait BeamBrokerKind: 'static {
    fn broker_url() -> Url;
    fn network_name() -> &'static str;
}

#[derive(Debug, Template)]
#[template(path = "beam.yml")]
pub struct BeamProxy<T: BeamBrokerKind> {
    kind: PhantomData<T>,
    proxy_id: String,
    app_keys: HashMap<&'static str, String>,
}

impl<T: BeamBrokerKind> BeamProxy<T> {
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

impl<T: BeamBrokerKind> Service for BeamProxy<T> {
    type Dependencies<'a> = ();

    fn from_config(conf: &Config, _: Self::Dependencies<'_>) -> Self {
        BeamProxy {
            kind: PhantomData,
            proxy_id: format!("{}.{}", conf.site_id, T::broker_url().host().unwrap()),
            app_keys: Default::default(),
        }
    }

    fn service_name() -> String {
        format!("{}-beam-proxy", T::network_name())
    }
}

pub struct DktkBroker;

impl BeamBrokerKind for DktkBroker {
    fn network_name() -> &'static str {
        "ccp"
    }

    fn broker_url() -> Url {
        Url::from_str("https://broker.example.com").unwrap()
    }
}
