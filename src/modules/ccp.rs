use std::str::FromStr;

use serde::Deserialize;
use url::Url;

use crate::services::{
    Blaze, BlazeProvider, BlazeTraefikConfig, BrokerProvider, Focus, IdManagement,
    IdManagementConfig, OidcProvider, ServiceMap,
};

use super::Module;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CcpConfig {
    pub id_manager: Option<IdManagementConfig>,
}

pub struct CcpDefault;

impl Module for CcpDefault {
    fn install(&self, service_map: &mut ServiceMap, conf: &crate::Config) {
        let Some(ccp_conf) = conf.ccp.as_ref() else {
            return;
        };
        service_map.install_with_config::<Focus<Self, Blaze<Self>>>(&"main-dktk".into());
        if ccp_conf.id_manager.is_some() {
            service_map.install_default::<IdManagement<Self>>();
        }
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
