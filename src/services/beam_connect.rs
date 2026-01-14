use std::{marker::PhantomData, path::PathBuf};

use askama::Template;
use serde::{Deserialize, Serialize};

use crate::{
    config::Config,
    services::{BeamAppInfos, BeamProxy, BrokerProvider, Service},
};

#[derive(Template)]
#[template(path = "beam_connect.yml")]

pub struct BeamConnect<T>
where
    Self: Service,
{
    beam: BeamAppInfos,
    trusted_ca_certs: PathBuf,
    local_targets: Vec<LocalTarget>,
    central_targets: Vec<CentralTarget>,
    pub no_proxy: Vec<String>,
    beam_provider: PhantomData<T>,
}

impl<T: BrokerProvider> Service for BeamConnect<T> {
    type Dependencies = (BeamProxy<T>,);

    type ServiceConfig = &'static Config;

    fn from_config(conf: Self::ServiceConfig, (beam,): super::Deps<Self>) -> Self {
        let beam = beam.add_service("beam-connect");
        Self {
            beam,
            trusted_ca_certs: conf.trusted_ca_certs(),
            local_targets: vec![],
            central_targets: vec![],
            no_proxy: vec![],
            beam_provider: PhantomData,
        }
    }

    fn service_name() -> String {
        format!("{}-beam-connect", T::network_name())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalTarget {
    /// The hostname used by the service talking to this one.
    pub external: String,
    /// The hostname that should be used internally to connect.
    pub internal: String,
    /// List of app or proxy IDs that are allowed to connect to this target.
    #[serde(default)]
    pub allowed: Vec<String>,
    /// Force the local connection to use HTTPS.
    #[serde(default, rename = "forceHttps")]
    pub force_https: bool,
    /// Removes the host header used by the incoming request causing the http client to use the internal hostname.
    #[serde(default, rename = "resetHost")]
    pub reset_host: bool,
    /// An additional regex that must match the path of the incoming request.
    /// This can be used to dispatch to a different target based on the path or for security purposes.
    #[serde(
        default,
        rename = "externalPathRegex",
        skip_serializing_if = "Option::is_none"
    )]
    pub external_path: Option<String>,
}

#[derive(Debug)]
pub struct CentralTarget {
    /// The hostname that gets mapped to the beam connect.
    pub virtualhost: String,
    /// The beam app id of the destination.
    pub beam_connect: String,
}

impl LocalTarget {
    pub fn new(external: String, internal: String, allowed: Vec<String>) -> Self {
        Self {
            external,
            internal,
            allowed,
            force_https: false,
            reset_host: false,
            external_path: None,
        }
    }
}

impl<T> BeamConnect<T>
where
    Self: Service,
{
    pub fn add_local_target(&mut self, target: LocalTarget) {
        self.local_targets.push(target);
    }

    pub fn add_central_target(&mut self, target: CentralTarget) {
        self.central_targets.push(target);
    }

    fn format_central_targets(&self) -> serde_json::Result<String> {
        let sites: Vec<_> = self
            .central_targets
            .iter()
            .map(|ct| {
                serde_json::json!({
                    "id": ct.virtualhost,
                    "name": ct.virtualhost,
                    "virtualhost": ct.virtualhost,
                    "beamconnect": ct.beam_connect
                })
            })
            .collect();
        serde_json::to_string_pretty(&serde_json::json!({
            "sites": sites
        }))
    }
}
