use std::{marker::PhantomData, str::FromStr};

use rinja::Template;
use url::Url;

use super::Service;


#[derive(Debug, Template)]
#[template(path = "blaze.yml")]
pub struct Blaze<For> where Self: Service {
    r#for: PhantomData<For>,
}

impl<For> Blaze<For> where Self: Service {
    pub fn get_url(&self) -> Url {
        Url::from_str(&format!("http://{}:8080", Self::service_name())).unwrap()
    }
}

impl<For: Service> Service for Blaze<For> {
    type Inputs<'s> = ();

    fn from_config(_conf: &crate::Config, _deps: super::Deps<'_, Self>) -> Self {
        Self { r#for: PhantomData }
    }

    fn service_name() -> String {
        format!("{}-blaze", <For as Service>::service_name())
    }
}

impl Service for Blaze<()> {
    type Inputs<'s> = ();

    fn from_config(_conf: &crate::Config, _deps: super::Deps<'_, Self>) -> Self {
        Self { r#for: PhantomData }
    }

    fn service_name() -> String {
        "blaze".into()
    }
}