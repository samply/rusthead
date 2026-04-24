use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    config::Config,
    modules::{Module, eucaim},
    services::{BrokerProvider, Focus, ServiceMap},
};

pub struct Eucaim;

#[derive(Debug, Deserialize, Clone)]
pub struct EucaimConfig {
    pub endpoint_type: EucaimEndpointType,
    pub provider: String,
    pub provider_icon: String,
    pub endpoint_url: Option<Url>,
    pub auth_header: Option<String>,
    pub postgres_connection_string: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum EucaimEndpointType {
    EucaimApi,
    EucaimSql,
    EucaimBeacon,
    Omop,
}

impl Module for Eucaim {
    fn install(&self, service_map: &mut ServiceMap, global_conf: &'static Config) {
        if let Some(eucaim_config) = global_conf.eucaim.clone() {
            service_map.install_with_config::<Focus<Eucaim, EucaimEndpointType>>(eucaim_config);
        }
    }
}

impl BrokerProvider for Eucaim {
    fn broker_url() -> url::Url {
        "https://broker.eucaim.cancerimage.eu".parse().unwrap()
    }

    fn network_name() -> &'static str {
        "eucaim"
    }

    fn root_cert() -> &'static str {
        include_str!("../../static/beam/eucaim.root.crt.pem")
    }
}
