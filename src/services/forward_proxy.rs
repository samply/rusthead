use std::{path::PathBuf, str::FromStr};

use askama::Template;
use url::Url;

use super::Service;
use crate::{config::Config, utils::filters};

#[derive(Debug, Template)]
#[template(path = "forward_proxy.yml")]
pub struct ForwardProxy {
    pub https_proxy_url: Option<Url>,
    trusted_ca_certs: PathBuf,
}

impl ForwardProxy {
    pub fn get_url(&self) -> Url {
        Url::from_str(&format!("http://{}:3128", Self::service_name())).unwrap()
    }
}

impl Service for ForwardProxy {
    type Dependencies = ();
    type ServiceConfig = &'static Config;

    fn from_config(conf: Self::ServiceConfig, _: super::Deps<Self>) -> Self {
        Self {
            https_proxy_url: conf.https_proxy_url.clone(),
            trusted_ca_certs: conf.trusted_ca_certs(),
        }
    }

    fn service_name() -> String {
        "forward-proxy".into()
    }
}
