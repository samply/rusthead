use std::{any::TypeId, marker::PhantomData};

use http::Uri;

use crate::{
    dep_map::{DepMap, ServiceMap},
    services::beam::DktkBeamProxy,
};

use super::{beam::BeamProxyKind, Service};

pub struct Focus<T: BeamProxyKind> {
    beam_id: String,
    beam_secret: String,
    beam_url: Uri,
    proxy: PhantomData<T>,
}

impl<T: BeamProxyKind> Service for Focus<T> {
    fn from_config(_conf: &crate::Config, deps: &mut ServiceMap) -> Self
    where
        Self: Sized,
    {
        let beam_proxy = deps.get_mut::<DktkBeamProxy>().unwrap();
        let (beam_id, beam_secret) = beam_proxy.add_service("focus");
        Focus {
            proxy: PhantomData,
            beam_id,
            beam_secret,
            beam_url: beam_proxy.get_url(),
        }
    }

    fn dependecies(deps: &mut DepMap) -> Vec<TypeId> {
        deps.ensure_installed::<DktkBeamProxy>();
        vec![DktkBeamProxy::type_id()]
    }

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
