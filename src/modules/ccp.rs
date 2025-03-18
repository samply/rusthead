use std::str::FromStr;

use url::Url;

use crate::services::{Blaze, BlazeProvider, BlazeTraefikConfig, BrokerProvider, Focus};

use super::Module;


pub struct CcpDefault;

impl Module for CcpDefault {
    fn enabled(&self, conf: &crate::Config) -> bool {
        conf.ccp.is_some()
    }

    fn install(&self, service_map: &mut crate::dep_map::ServiceMap, conf: &crate::Config) {
        service_map.install::<Focus<Self, Blaze<Self>>>(conf);
    }
}

impl BlazeProvider for CcpDefault {
    fn balze_service_name() -> String {
        "ccp-blaze".into()
    }

    fn treafik_exposure() -> Option<BlazeTraefikConfig> {
        Some(BlazeTraefikConfig {
            path: "/ccp-localdatamanagement".into(),
            user: "ccp".into(),
        })
    }
}

impl BrokerProvider for CcpDefault {
    fn network_name() -> &'static str {
        "ccp"
    }

    fn broker_url() -> Url {
        Url::from_str("https://broker.example.com").unwrap()
    }
}