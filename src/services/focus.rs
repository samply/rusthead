use std::marker::PhantomData;

use askama::Template;
use url::Url;

use crate::Config;

use super::{
    Deps, Service,
    beam::{BeamProxy, BrokerProvider},
    blaze::{Blaze, BlazeProvider},
};

#[derive(Debug, Template)]
#[template(path = "focus.yml")]
pub struct Focus<Beam: BrokerProvider, Backend>
where
    Self: Service,
{
    beam_id: String,
    beam_secret: String,
    beam_url: Url,
    blaze_url: Url,
    endpoint_type: String,
    pub tag: String,
    beam_and_blaze: PhantomData<(Beam, Backend)>,
}

impl<T: BrokerProvider, B: BlazeProvider> Service for Focus<T, Blaze<B>> {
    type Dependencies = (BeamProxy<T>, Blaze<B>);

    fn from_config(_conf: &Config, (beam_proxy, blaze): Deps<Self>) -> Self
    where
        Self: Sized,
    {
        let (beam_id, beam_secret) = beam_proxy.add_service("focus");
        Focus {
            beam_and_blaze: PhantomData,
            beam_id,
            beam_secret,
            beam_url: beam_proxy.get_url(),
            blaze_url: blaze.get_url(),
            tag: "main".into(),
            endpoint_type: "blaze".into(),
        }
    }

    fn service_name() -> String {
        format!("{}-focus", T::network_name())
    }
}
