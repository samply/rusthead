use std::str::FromStr;

use serde::Deserialize;
use url::Url;

use crate::{
    config::{Config, Environment},
    services::{
        Blaze, BlazeConfig, BlazeProvider, BlazeTraefikConfig, BrokerProvider, Exporter, Focus,
        FocusConfig, OidcProvider, ServiceMap, Teiler, TeilerConfig,
    },
    utils::capitalize_first_letter,
};

use super::Module;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CceConfig {
    // id_manager: Option<Empty>,
    // transfair: Option<Empty>,
    teiler: Option<TeilerConfig>,
    exporter: Option<Empty>,
    // datashield: Option<Empty>,
    // obds2fhir: Option<Empty>,
    // podest2fhir: Option<Empty>,
    blaze: Option<BlazeConfig>,
}

#[derive(Debug, Deserialize)]
struct Empty {}

pub struct Cce;
pub struct CceTest;

impl Module for Cce {
    fn install(&self, service_map: &mut ServiceMap, conf: &'static Config) {
        let Some(cce_conf) = conf.cce.as_ref() else {
            return;
        };
        if let Environment::Test = conf.environment {
            // The shared Samply test broker has no central OIDC secret-sync for the
            // CCE IdP, so OIDC-backed services cannot enroll there — the test
            // environment only exercises beam connectivity via Focus.
            if cce_conf.teiler.is_some() || cce_conf.exporter.is_some() {
                eprintln!(
                    "cce: skipping teiler/exporter in the test environment (no OIDC secret-sync on the test broker)"
                );
            }
            service_map
                .install_with_config::<Focus<CceTest, Blaze<Self>>>(FocusConfig::from_tag("main"));
            return;
        }
        service_map.install_with_config::<Focus<Self, Blaze<Self>>>(FocusConfig::from_tag("main"));
        if let Some(teiler_conf) = &cce_conf.teiler {
            service_map.install_with_config::<Teiler<Self>>((teiler_conf, conf));
        }
        if let Some(Empty {}) = &cce_conf.exporter {
            service_map.install_default::<Exporter<Self>>();
        }
    }
}

impl BlazeProvider for Cce {
    fn balze_service_name() -> String {
        "cce-blaze".into()
    }

    fn treafik_exposure() -> Option<BlazeTraefikConfig> {
        Some(BlazeTraefikConfig {
            path: "/cce-localdatamanagement".into(),
            middleware_and_user_name: "cce-blaze".into(),
        })
    }

    fn blaze_config(conf: &Config) -> Option<BlazeConfig> {
        conf.cce.as_ref()?.blaze.clone()
    }
}

impl OidcProvider for Cce {
    type BeamProvider = Self;

    fn oidc_provider_id() -> String {
        format!(
            "secret-sync-central.central-secret-sync.{}",
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
        // TODO: Check if the below constant is correct
        format!("CCE_{}_Verwalter", capitalize_first_letter(&conf.site_id))
    }
}

impl BrokerProvider for Cce {
    fn broker_url() -> url::Url {
        Url::from_str("https://broker.cce.dkfz.de").unwrap()
    }

    fn network_name() -> &'static str {
        "cce"
    }

    fn root_cert() -> &'static str {
        include_str!("../../static/beam/cce.root.crt.pem")
    }
}

impl BrokerProvider for CceTest {
    fn broker_url() -> url::Url {
        Url::from_str("https://test-no-real-data.broker.samply.de").unwrap()
    }

    fn network_name() -> &'static str {
        "cce-test"
    }

    fn root_cert() -> &'static str {
        include_str!("../../static/beam/cce.test.root.crt.pem")
    }
}
