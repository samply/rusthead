use std::{collections::HashMap, marker::PhantomData};

use http::Uri;

use crate::{dep_map::Constructor, utils::generate_password, Config};

use super::ToCompose;

pub trait BeamProxyKind {
    const BROKER_URL_STR: &str;

    fn broker_url() -> Uri {
        Uri::from_static(Self::BROKER_URL_STR)
    }
}

#[derive(Debug)]
pub struct BeamProxy<T: BeamProxyKind> {
    kind: PhantomData<T>,
    proxy_id: String,
    app_keys: HashMap<&'static str, String>,
}
impl<T: BeamProxyKind> BeamProxy<T> {
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

fn make_beam_proxy<T: BeamProxyKind>(conf: &Config) -> BeamProxy<T> {
    BeamProxy {
        kind: PhantomData,
        proxy_id: format!("{}.{}", conf.site_id, T::broker_url().host().unwrap()),
        app_keys: Default::default(),
    }
}

inventory::submit! {
    Constructor::new::<DktkBeamProxy>(&(make_beam_proxy as fn(&Config) -> DktkBeamProxy))
}

pub struct DktkBroker;

impl BeamProxyKind for DktkBroker {
    const BROKER_URL_STR: &str = "https://asf.const";
}

pub type DktkBeamProxy = BeamProxy<DktkBroker>;

impl<T: BeamProxyKind> ToCompose for BeamProxy<T> {
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
