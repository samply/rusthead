use std::{path::PathBuf, str::FromStr};

use rinja::Template;
use url::Url;

use super::Service;
use crate::utils::filters;

#[derive(Debug, Template)]
#[template(path = "forward_proxy.yml")]
pub struct ForwardProxy {
    https_proxy_url: Option<Url>,
    trusted_ca_certs: PathBuf,
}

impl ForwardProxy {
    pub fn get_url(&self) -> Url {
        Url::from_str(&format!("http://{}:3128", Self::service_name())).unwrap()
    }
}

impl Service for ForwardProxy {
    type Dependencies<'s> = ();

    fn from_config(conf: &crate::config::Config, _: super::Deps<'_, Self>) -> Self {
        Self {
            https_proxy_url: conf.https_proxy_url.clone(),
            trusted_ca_certs: conf.path.join("trusted-ca-certs"),
        }
    }

    fn service_name() -> String {
        "forward-proxy".into()
    }
}
