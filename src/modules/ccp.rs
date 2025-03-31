use std::str::FromStr;

use url::Url;

use crate::services::{
    Blaze, BlazeProvider, BlazeTraefikConfig, BrokerProvider, Focus, OidcProvider, ServiceMap,
};

use super::Module;

pub struct CcpDefault;

impl Module for CcpDefault {
    fn enabled(&self, conf: &crate::Config) -> bool {
        conf.ccp.is_some()
    }

    fn install(&self, service_map: &mut ServiceMap, conf: &crate::Config) {
        service_map.install::<Focus<Self, Blaze<Self>>>(conf);
    }
}

impl BlazeProvider for CcpDefault {
    fn balze_service_name() -> String {
        "ccp-blaze".into()
    }

    fn treafik_exposure() -> Option<BlazeTraefikConfig> {
        Some(BlazeTraefikConfig {
            middleware_and_user_name: "ccp-blaze".into(),
            path: "/ccp-localdatamanagement".into(),
        })
    }
}

impl BrokerProvider for CcpDefault {
    fn network_name() -> &'static str {
        "ccp"
    }

    fn broker_url() -> Url {
        Url::from_str("https://broker.ccp-it.dktk.dkfz.de").unwrap()
    }

    fn root_cert() -> &'static str {
        include_str!("../../static/beam/ccp.root.crt.pem")
    }
}

impl OidcProvider for CcpDefault {
    type BeamProvider = Self;

    fn oidc_provider_id() -> String {
        format!(
            "secret-sync-central.central-secret-sync.{}",
            Self::BeamProvider::broker_id()
        )
    }

    fn issuer_url() -> Url {
        Url::parse("https://login.verbis.dkfz.de/realms/test-realm-01").unwrap()
    }
}
