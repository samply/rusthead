use std::marker::PhantomData;

use askama::Template;
use serde::Deserialize;
use url::Url;

use crate::{
    config::Config,
    modules::CcpDefault,
    services::{IdManagement, Service, Traefik},
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

impl Obds2FhirConfig {
    pub fn defaulted_with<T>(&self, ml: &IdManagement<T>, blaze_url: Url) -> Self
    where
        IdManagement<T>: Service,
    {
        let mut this = self.clone();
        this.id_type
            .get_or_insert_with(|| format!("BK_{}_L-ID", ml.id));
        this.mainzelliste_apikey
            .get_or_insert_with(|| ml.local_apikey.clone());
        this.mainzelliste_url.get_or_insert_with(|| {
            Url::parse(&format!(
                "http://{}:{}",
                <IdManagement<T> as Service>::service_name(),
                8080
            ))
            .unwrap()
            .join("/patientlist")
            .unwrap()
        });
        this.fhir_server_url.get_or_insert(blaze_url);
        this
    }
}

#[derive(Debug, Template)]
#[template(path = "obds2fhir.yml")]
pub struct Obds2Fhir<T: Obds2FhirProvider> {
    /// This should include /fhir
    fhir_server_url: Url,
    /// This should include /patientlist
    mainzelliste_url: Url,
    mainzelliste_apikey: String,
    id_type: String,
    keep_internal_id: bool,
    salt: String,
    middleware_name: String,
    provider: PhantomData<T>,
}

impl<T: Obds2FhirProvider> Service for Obds2Fhir<T> {
    type Dependencies = (Traefik,);

    type ServiceConfig = (Obds2FhirConfig, &'static Config);

    fn from_config((obds_conf, conf): Self::ServiceConfig, (traefik,): super::Deps<Self>) -> Self {
        let middleware_name = format!("{}-obds2fhir-auth", T::prefix());
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
                .expect("obds2fhir needs a fhir server url"),
            mainzelliste_url: obds_conf
                .mainzelliste_url
                .expect("obds2fhir needs a mainzelliste url"),
            mainzelliste_apikey: obds_conf
                .mainzelliste_apikey
                .expect("obds2fhir needs a mainzelliste apikey"),
            id_type: obds_conf.id_type.expect("obds2fhir needs an id type"),
            keep_internal_id: obds_conf.keep_internal_id,
            provider: PhantomData,
        }
    }

    fn service_name() -> String {
        format!("{}-obds2fhir-rest", T::prefix())
    }
}

pub trait Obds2FhirProvider: 'static {
    fn prefix() -> &'static str;
}

impl Obds2FhirProvider for CcpDefault {
    fn prefix() -> &'static str {
        "ccp"
    }
}
