use std::marker::PhantomData;

use http::Uri;

use crate::Config;

use super::{
    beam::{BeamBrokerKind, BeamProxy},
    Deps, Service, ToCompose,
};

pub struct Focus<T: BeamBrokerKind> {
    beam_id: String,
    beam_secret: String,
    beam_url: Uri,
    proxy: PhantomData<T>,
}

impl<T: BeamBrokerKind> ToCompose for Focus<T> {
    #[rustfmt::skip]
    fn to_compose(&self) -> serde_yaml::Value {
        let Self { beam_id, beam_secret, beam_url, proxy: _ } = self;
        serde_yaml::from_str(&format!(r###"
        focus:
          image: samply/focus
          environment:
            BEAM_ID: {beam_id}
            BEAM_URL: {beam_url}
            BEAM_SECRET: "{beam_secret}"
        "###)).unwrap()
    }
}

impl<T: BeamBrokerKind + 'static> Service for Focus<T> {
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
}
