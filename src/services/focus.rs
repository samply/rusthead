use std::marker::PhantomData;

use askama::Template;
use url::Url;

use crate::{
    modules::{EucaimConfig, EucaimEndpointType},
    services::BeamAppInfos,
};

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
    endpoint_url: Option<Url>,
    endpoint_type: String,
    tag: String,
    /// (exporter_url, exporter_api_key)
    exporter: Option<(String, String)>,
    beam_and_blaze: PhantomData<(Beam, Backend)>,
    /// (provider, provider_icon)
    provider_data: Option<(String, String)>,
    auth_header: Option<String>,
    postgres_connection_string: Option<String>,
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
            endpoint_url: Some(Blaze::<B>::get_url()),
            tag: tag.clone(),
            endpoint_type: "blaze".into(),
            exporter: None,
            provider_data: None,
            auth_header: None,
            postgres_connection_string: None,
        }
    }

    fn service_name() -> String {
        format!("{}-focus", T::network_name())
    }
}

// only going to be used for Eucaim
impl<T: BrokerProvider> Service for Focus<T, EucaimEndpointType> {
    type Dependencies = (BeamProxy<T>,);
    type ServiceConfig = EucaimConfig;

    fn from_config(config: Self::ServiceConfig, (beam_proxy,): Deps<Self>) -> Self
    where
        Self: Sized,
    {
        let beam = beam_proxy.add_service("focus");
        Focus {
            beam,
            beam_and_blaze: PhantomData,
            endpoint_url: config.endpoint_url,
            tag: "develop".to_string(),
            endpoint_type: serde_json::to_string(&config.endpoint_type).unwrap(),
            exporter: None,
            provider_data: Some((config.provider, config.provider_icon)),
            auth_header: config.auth_header,
            postgres_connection_string: config.postgres_connection_string,
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
