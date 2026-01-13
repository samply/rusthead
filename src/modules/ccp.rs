use std::str::FromStr;

use serde::Deserialize;
use url::Url;

use crate::{
    config::Config,
    services::{
        Blaze, BlazeProvider, BlazeTraefikConfig, BrokerProvider, DataShield, Exporter, Focus,
        IdManagement, IdManagementConfig, OidcProvider, ServiceMap, Teiler, TeilerConfig,
        Transfair, TransfairConfig,
        obds2fhir::{Obds2Fhir, Obds2FhirConfig},
    },
    utils::capitalize_first_letter,
};

use super::Module;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CcpConfig {
    id_manager: Option<IdManagementConfig>,
    transfair: Option<TransfairConfig>,
    teiler: Option<TeilerConfig>,
    exporter: Option<Empty>,
    datashield: Option<Empty>,
    obds2fhir: Option<Obds2FhirConfig>,
}

#[derive(Debug, Deserialize)]
struct Empty {}

pub struct CcpDefault;

impl Module for CcpDefault {
    fn install(&self, service_map: &mut ServiceMap, conf: &'static crate::Config) {
        let Some(ccp_conf) = conf.ccp.as_ref() else {
            return;
        };
        service_map.install_with_config::<Focus<Self, Blaze<Self>>>("main-dktk".into());
        if let Some(idm_conf) = &ccp_conf.id_manager {
            let ml = service_map.install_with_config::<IdManagement<Self>>((idm_conf, conf));
            if let Some(obds_conf) = &ccp_conf.obds2fhir {
                let blaze_url = Blaze::<Self>::get_url().join("fhir").unwrap();
                let obds_conf = obds_conf.defaulted_with(ml, blaze_url);
                service_map.install_with_config::<Obds2Fhir<Self>>((obds_conf, conf));
            }
        } else if ccp_conf.obds2fhir.is_some() {
            panic!("obds2fhir rest requires id mamanger setup")
        }
        if let Some(transfair_conf) = &ccp_conf.transfair {
            service_map.install_with_config::<Transfair<Self>>((transfair_conf, conf));
        }
        if let Some(Empty {}) = &ccp_conf.datashield {
            service_map.install_default::<DataShield<Self>>();
        }
        if let Some(Empty {}) = &ccp_conf.exporter {
            service_map.install_default::<Exporter<Self>>();
        }
        if let Some(teiler_conf) = &ccp_conf.teiler {
            service_map.install_with_config::<Teiler<Self>>((teiler_conf, conf));
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
            "secret-sync-central.test-secret-sync.{}",
            Self::BeamProvider::broker_id()
        )
    }

    fn issuer_url(public_client_id: &str) -> Url {
        Url::parse(&format!(
            "https://sso.verbis.dkfz.de/application/o/{public_client_id}/"
        ))
        .unwrap()
    }

    fn private_issuer_url(private_client_id: &str) -> Url {
        Url::parse(&format!(
            "https://sso.verbis.dkfz.de/application/o/{private_client_id}/"
        ))
        .unwrap()
    }

    fn admin_group(conf: &Config) -> String {
        format!(
            "DKTK_CCP_{}_Verwalter",
            capitalize_first_letter(&conf.site_id)
        )
    }
}
