use std::{marker::PhantomData, str::FromStr};

use askama::Template;
use url::Url;

use super::{Service, Traefik};

#[derive(Debug, Template)]
#[template(path = "blaze.yml")]
pub struct Blaze<T>
where
    Self: Service,
{
    r#for: PhantomData<T>,
    traefik_conf: Option<BlazeTraefikConfig>,
}

impl<T> Blaze<T>
where
    Self: Service,
{
    pub fn get_url(&self) -> Url {
        Url::from_str(&format!("http://{}:8080", Self::service_name())).unwrap()
    }
}

impl<T: BlazeProvider> Service for Blaze<T> {
    type Dependencies = (Traefik,);
    type ServiceConfig = ();

    fn from_config(_conf: Self::ServiceConfig, (traefik,): super::Deps<Self>) -> Self {
        let traefik_conf = T::treafik_exposure();
        if let Some(conf) = &traefik_conf {
            traefik.add_basic_auth_user(conf.middleware_and_user_name.clone())
        }
        Self {
            r#for: PhantomData,
            traefik_conf,
        }
    }

    fn service_name() -> String {
        T::balze_service_name()
    }
}

pub trait BlazeProvider: 'static {
    fn balze_service_name() -> String;

    /// relative path where this balze should be exposed thorugh traefik. Defaults to None
    fn treafik_exposure() -> Option<BlazeTraefikConfig> {
        None
    }
}

#[derive(Debug)]
pub struct BlazeTraefikConfig {
    pub path: String,
    pub middleware_and_user_name: String,
}

impl<T: Service> BlazeProvider for T {
    fn balze_service_name() -> String {
        format!("{}-blaze", <T as Service>::service_name())
    }
}
