use std::marker::PhantomData;

use rinja::Template;
use url::Url;

use crate::Config;

use super::{
    beam::{BeamProxy, BrokerProvider},
    blaze::{Blaze, BlazeProvider},
    Deps, Service,
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
    beam_and_blaze: PhantomData<(Beam, Backend)>,
}

impl<T: BrokerProvider, B: BlazeProvider> Service for Focus<T, Blaze<B>> {
    type Dependencies<'a> = (BeamProxy<T>, Blaze<B>);

    fn from_config(_conf: &Config, (beam_proxy, blaze): Deps<'_, Self>) -> Self
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
        }
    }

    fn service_name() -> String {
        format!("{}-focus", T::network_name())
    }
}
