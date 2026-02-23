use std::marker::PhantomData;

use askama::Template;
use serde::Deserialize;
use url::Url;

use crate::{
    config::Config,
    services::{Blaze, BlazeProvider, BrokerProvider, Service},
};

#[derive(Debug, Deserialize, Clone)]
pub struct Podest2FhirConfig {
    /// Override the FHIR base URL (defaults to the bundled Blaze instance)
    pub fhir_base_url: Option<Url>,
    pub db_host: String,
    pub db_port: u16,
    pub db_name: String,
    pub db_user: String,
    pub db_password: String,
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
    db_password: String,
    profile: &'static str,
    kind: PhantomData<T>,
}

impl<T: BrokerProvider + BlazeProvider> Service for Podest2Fhir<T>
where
    Podest2Fhir<T>: 'static,
{
    type Dependencies = (Blaze<T>,);

    type ServiceConfig = (Podest2FhirConfig, &'static str, &'static Config);

    fn from_config(
        (conf, profile, _global): Self::ServiceConfig,
        (_blaze,): super::Deps<Self>,
    ) -> Self {
        Self {
            fhir_base_url: conf
                .fhir_base_url
                .unwrap_or_else(|| Blaze::<T>::get_url().join("/fhir").unwrap()),
            db_host: conf.db_host,
            db_port: conf.db_port,
            db_name: conf.db_name,
            db_user: conf.db_user,
            db_password: conf.db_password,
            profile,
            kind: PhantomData,
        }
    }

    fn service_name() -> String {
        format!("{}-podest2fhir", T::network_name())
    }
}
