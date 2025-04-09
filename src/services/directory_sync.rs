use std::marker::PhantomData;

use askama::Template;
use serde::Deserialize;
use url::Url;

use crate::utils::enabled;

use super::{Blaze, BlazeProvider, Deps, Service};

#[derive(Debug, Deserialize, Clone)]
pub struct DirectorySyncConfig {
    username: String,
    password: String,
    #[serde(default = "default_directory_sync_url")]
    url: Url,
    #[serde(default = "default_directory_sync_cron")]
    timer_cron: String,
    #[serde(default = "enabled")]
    allow_star_model: bool,
    #[serde(default)]
    mock: String,
    #[serde(default)]
    default_collection_id: String,
    #[serde(default)]
    country: String,
}

fn default_directory_sync_url() -> Url {
    "https://directory.bbmri-eric.eu".parse().unwrap()
}

fn default_directory_sync_cron() -> String {
    "0 22 * * *".to_owned()
}

#[derive(Debug, Template)]
#[template(path = "directory_sync.yml")]
pub struct DirectorySync<T: BlazeProvider> {
    conf: DirectorySyncConfig,
    blaze_url: Url,
    blaze_provider: PhantomData<T>,
}

impl<T: BlazeProvider> Service for DirectorySync<T> {
    type Dependencies = (Blaze<T>,);
    type ServiceConfig = DirectorySyncConfig;

    fn service_name() -> String {
        format!("{}-directory-sync", T::balze_service_name())
    }

    fn from_config(conf: &Self::ServiceConfig, (blaze,): Deps<Self>) -> Self {
        DirectorySync {
            blaze_url: blaze.get_url(),
            conf: conf.clone(),
            blaze_provider: PhantomData,
        }
    }
}
