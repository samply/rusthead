use std::marker::PhantomData;

use http::Uri;
use rinja::Template;

use crate::Config;

use super::{
    beam::{BeamBrokerKind, BeamProxy},
    Deps, Service,
};

#[derive(Debug, Template)]
#[template(path = "focus.yml")]
pub struct Focus<T: BeamBrokerKind> {
    beam_id: String,
    beam_secret: String,
    beam_url: Uri,
    proxy: PhantomData<T>,
}

impl<T: BeamBrokerKind> Service for Focus<T> {
    type Inputs<'a> = (BeamProxy<T>,);

    fn from_config(_conf: &Config, (beam_proxy,): Deps<'_, Self>) -> Self
    where
        Self: Sized,
    {
        let (beam_id, beam_secret) = beam_proxy.add_service("focus");
        Focus {
            proxy: PhantomData,
            beam_id,
            beam_secret,
            beam_url: beam_proxy.get_url(),
        }
    }

    fn service_name() -> String {
        format!("{}-focus", T::network_name())
    }
}
