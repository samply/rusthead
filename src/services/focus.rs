use std::marker::PhantomData;

use askama::Template;
use url::Url;

use crate::services::BeamAppInfos;

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
    beam: BeamAppInfos,
    blaze_url: Url,
    endpoint_type: String,
    tag: String,
    /// (exporter_url, exporter_api_key)
    exporter: Option<(String, String)>,
    beam_and_blaze: PhantomData<(Beam, Backend)>,
}

impl<T: BrokerProvider, B: BlazeProvider> Service for Focus<T, Blaze<B>> {
    type Dependencies = (BeamProxy<T>, Blaze<B>);
    type ServiceConfig = String;

    fn from_config(tag: Self::ServiceConfig, (beam_proxy, _blaze): Deps<Self>) -> Self
    where
        Self: Sized,
    {
        let beam = beam_proxy.add_service("focus");
        Focus {
            beam,
            beam_and_blaze: PhantomData,
            blaze_url: Blaze::<B>::get_url(),
            tag: tag.clone(),
            endpoint_type: "blaze".into(),
            exporter: None,
        }
    }

    fn service_name() -> String {
        format!("{}-focus", T::network_name())
    }
}

impl<Beam: BrokerProvider, Backend> Focus<Beam, Backend>
where
    Self: Service,
{
    pub fn enable_exporter(&mut self, exporter_url: String, exporter_api_key: String) {
        self.exporter = Some((exporter_url, exporter_api_key));
    }
}
