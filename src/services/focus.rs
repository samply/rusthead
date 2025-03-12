use std::marker::PhantomData;

use rinja::Template;
use url::Url;

use crate::Config;

use super::{
    beam::{BeamBrokerKind, BeamProxy}, blaze::Blaze, Deps, Service
};

#[derive(Debug, Template)]
#[template(path = "focus.yml")]
pub struct Focus<T: BeamBrokerKind> {
    beam_id: String,
    beam_secret: String,
    beam_url: Url,
    blaze_url: Url,
    proxy: PhantomData<T>,
}

impl<T: BeamBrokerKind> Service for Focus<T> {
    type Dependencies<'a> = (BeamProxy<T>, Blaze<Self>);

    fn from_config(_conf: &Config, (beam_proxy, blaze): Deps<'_, Self>) -> Self
    where
        Self: Sized,
    {
        let (beam_id, beam_secret) = beam_proxy.add_service("focus");
        Focus {
            proxy: PhantomData,
            beam_id,
            beam_secret,
            beam_url: beam_proxy.get_url(),
            blaze_url: blaze.get_url(),
        }
    }

    fn service_name() -> String {
        format!("{}-focus", T::network_name())
    }
}
