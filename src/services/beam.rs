use std::{collections::HashMap, marker::PhantomData};

use http::Uri;

use crate::{utils::generate_password, Config};

use super::{Service, ToCompose};

pub trait BeamBrokerKind {
    const BROKER_URL_STR: &str;

    fn broker_url() -> Uri {
        Uri::from_static(Self::BROKER_URL_STR)
    }
}

#[derive(Debug)]
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

    pub fn get_url(&self) -> Uri {
        Uri::from_static("http://beam-proxy")
    }
}

impl<T: BeamBrokerKind + 'static> Service for BeamProxy<T> {
    type Inputs<'a> = ();

    fn from_config(conf: &Config, _: Self::Inputs<'_>) -> Self {
        BeamProxy {
            kind: PhantomData,
            proxy_id: format!("{}.{}", conf.site_id, T::broker_url().host().unwrap()),
            app_keys: Default::default(),
        }
    }
}

pub struct DktkBroker;

impl BeamBrokerKind for DktkBroker {
    const BROKER_URL_STR: &str = "https://asf.const";
}

pub type DktkBeamProxy = BeamProxy<DktkBroker>;

impl<T: BeamBrokerKind> ToCompose for BeamProxy<T> {
    #[rustfmt::skip]
    fn to_compose(&self) -> serde_yaml::Value {
        let Self { kind: _, proxy_id, app_keys } = self;
        let broker_url = T::broker_url();
        let mut yaml: serde_yaml::Value = serde_yaml::from_str(&format!(r###"
        beam-proxy:
          image: samply/beam-proxy
          environment:
            BROKER_URL: {broker_url}
            PROXY_ID: {proxy_id}
        "###
        ))
        .unwrap();
        let envs = yaml["beam-proxy"]["environment"].as_mapping_mut().unwrap();
        for (app_name, secret) in app_keys {
            envs.insert(format!("APP_{app_name}_KEY").into(), secret.clone().into());
        }
        yaml
    }
}
