use std::{marker::PhantomData, str::FromStr};

use rinja::Template;
use url::Url;

use super::{Traefik, Service};


#[derive(Debug, Template)]
#[template(path = "blaze.yml")]
pub struct Blaze<T> where Self: Service {
    r#for: PhantomData<T>,
    traefik_conf: Option<BlazeTraefikConfig>,
}

impl<T> Blaze<T> where Self: Service {
    pub fn get_url(&self) -> Url {
        Url::from_str(&format!("http://{}:8080", Self::balze_service_name())).unwrap()
    }
}

impl<T: BlazeProvider> Service for Blaze<T> {
    type Dependencies<'s> = (Traefik, );

    fn from_config(_conf: &crate::Config, (_traefik,): super::Deps<'_, Self>) -> Self {
        let traefik_conf = T::treafik_exposure();
        // TODO:
        // if let Some(conf) = traefik_conf {
        //     traefik.add_basic_auth_user(conf.user)
        // }
        Self { r#for: PhantomData, traefik_conf }
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
    pub user: String,
}

impl<T: Service> BlazeProvider for T {
    fn balze_service_name() -> String {
        format!("{}-blaze", <T as Service>::service_name())
    }
}