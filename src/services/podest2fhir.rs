use std::marker::PhantomData;

use askama::Template;
use serde::Deserialize;
use url::Url;

use crate::services::{Blaze, BlazeProvider, BrokerProvider, Focus, Service};

#[derive(Debug, Deserialize, Clone)]
pub struct Podest2FhirConfig {
    /// Override the FHIR base URL (defaults to the bundled Blaze instance)
    pub fhir_base_url: Option<Url>,
    pub db_host: String,
    pub db_port: u16,
    pub db_name: String,
    pub db_user: String,
}

#[derive(Debug, Template)]
#[template(path = "podest2fhir.yml")]
pub struct Podest2Fhir<T>
where
    Self: Service,
{
    fhir_base_url: Url,
    db_host: String,
    db_port: u16,
    db_name: String,
    db_user: String,
    profile: &'static str,
    kind: PhantomData<T>,
}

impl<T: BrokerProvider + BlazeProvider> Service for Podest2Fhir<T>
where
    Podest2Fhir<T>: 'static,
{
    type Dependencies = (Blaze<T>, Focus<T, Blaze<T>>);

    type ServiceConfig = (Podest2FhirConfig, &'static str);

    fn from_config(
        (conf, profile): Self::ServiceConfig,
        (_blaze, _focus): super::Deps<Self>,
    ) -> Self {
        Self {
            fhir_base_url: conf
                .fhir_base_url
                .unwrap_or_else(|| Blaze::<T>::get_url().join("/fhir").unwrap()),
            db_host: conf.db_host,
            db_port: conf.db_port,
            db_name: conf.db_name,
            db_user: conf.db_user,
            profile,
            kind: PhantomData,
        }
    }

    fn service_name() -> String {
        format!("{}-podest2fhir", T::network_name())
    }
}
