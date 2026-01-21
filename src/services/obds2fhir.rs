use std::marker::PhantomData;

use askama::Template;
use serde::Deserialize;
use url::Url;

use crate::{
    config::Config,
    services::{Blaze, BlazeProvider, BrokerProvider, IdManagement, Service, Traefik},
};

#[derive(Debug, Deserialize, Clone)]
pub struct Obds2FhirConfig {
    /// This should include /fhir
    pub fhir_server_url: Option<Url>,
    /// This should include /patientlist
    pub mainzelliste_url: Option<Url>,
    pub mainzelliste_apikey: Option<String>,
    pub id_type: Option<String>,
    #[serde(default)]
    pub keep_internal_id: bool,
}

#[derive(Debug, Template)]
#[template(path = "obds2fhir.yml")]
pub struct Obds2Fhir<T>
where
    Self: Service,
{
    /// This should include /fhir
    fhir_server_url: Url,
    /// This should include /patientlist
    mainzelliste_url: Url,
    mainzelliste_apikey: String,
    id_type: String,
    keep_internal_id: bool,
    salt: String,
    middleware_name: String,
    prefix: String,
    kind: PhantomData<T>,
}

impl<T: BrokerProvider + BlazeProvider> Service for Obds2Fhir<IdManagement<T>>
where
    IdManagement<T>: Service,
{
    type Dependencies = (Traefik, IdManagement<T>);

    type ServiceConfig = (Obds2FhirConfig, &'static Config);

    fn from_config(
        (obds_conf, conf): Self::ServiceConfig,
        (traefik, ml): super::Deps<Self>,
    ) -> Self {
        let middleware_name = format!("{}-obds2fhir-auth", T::network_name());
        traefik.add_basic_auth_user(middleware_name.clone());
        let salt = conf
            .local_conf
            .borrow_mut()
            .generate_secret::<30, Self>("salt");
        Self {
            salt,
            middleware_name,
            fhir_server_url: obds_conf
                .fhir_server_url
                .unwrap_or_else(|| Blaze::<T>::get_url().join("fhir").unwrap()),
            mainzelliste_url: obds_conf.mainzelliste_url.unwrap_or_else(|| {
                Url::parse(&format!(
                    "http://{}:{}",
                    IdManagement::<T>::service_name(),
                    8080
                ))
                .unwrap()
                .join("/patientlist")
                .unwrap()
            }),
            mainzelliste_apikey: obds_conf
                .mainzelliste_apikey
                .unwrap_or_else(|| ml.local_apikey.clone()),
            id_type: obds_conf
                .id_type
                .unwrap_or_else(|| format!("BK_{}_L-ID", ml.id)),
            keep_internal_id: obds_conf.keep_internal_id,
            prefix: T::network_name().to_string(),
            kind: PhantomData,
        }
    }

    fn service_name() -> String {
        format!("{}-obds2fhir-rest", T::network_name())
    }
}
