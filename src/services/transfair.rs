use std::{marker::PhantomData, path::PathBuf};

use askama::Template;
use serde::Deserialize;
use url::Url;

use super::{Blaze, BlazeProvider, Service};

#[derive(Debug, Deserialize)]
pub struct TransfairConfig {
    ttp: Option<TransfairTtpConfig>,
    fhir_requests: Option<FhirServerConfig>,
    fhir_output: Option<FhirServerConfig>,
    fhir_input: Option<FhirServerConfig>,
    #[serde(default = "default_exchange_id_system")]
    exchange_id_system: String,
    #[serde(default)]
    tls_disable: bool,
}

#[derive(Debug, Deserialize, Clone)]
struct FhirServerConfig {
    url: Url,
    #[serde(default)]
    auth: String,
}

fn default_exchange_id_system() -> String {
    "SESSION_ID".to_string()
}

#[derive(Debug, Deserialize, Clone)]
pub struct TransfairTtpConfig {
    url: Url,
    #[serde(default)]
    auth: String,
    project_id_system: String,
    #[serde(flatten)]
    ttp_type: TtpType,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum TtpType {
    Greifswald {
        source: String,
        epix_domain: String,
        gpas_domain: String,
    },
    Mainzelliste {
        apikey: String,
    },
}

#[derive(Debug, Template)]
#[template(path = "transfair.yml")]
pub struct Transfair<T>
where
    Self: Service,
{
    provider: PhantomData<T>,
    conf: &'static TransfairConfig,
    fhir_out_server: FhirServerConfig,
    trusted_ca_certs: PathBuf,
}

impl<T: BlazeProvider> Service for Transfair<T> {
    type Dependencies = (Blaze<T>,);
    type ServiceConfig = (&'static TransfairConfig, &'static crate::Config);

    fn from_config((conf, global_conf): Self::ServiceConfig, (blaze,): super::Deps<Self>) -> Self {
        Self {
            provider: PhantomData,
            conf,
            fhir_out_server: conf
                .fhir_output
                .clone()
                .unwrap_or_else(|| FhirServerConfig {
                    url: blaze.get_url(),
                    auth: "".to_string(),
                }),
            trusted_ca_certs: global_conf.trusted_ca_certs(),
        }
    }

    fn service_name() -> String {
        "transfair".into()
    }
}
