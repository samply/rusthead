use std::{marker::PhantomData, str::FromStr};

use askama::Template;
use serde::Deserialize;
use url::Url;

use crate::{config::Config, utils::filters};

use super::{Service, Traefik};

/// Performance tuning for a Blaze instance.
///
/// Sizing guidance: <https://github.com/samply/blaze/blob/main/docs/production-configuration.md>
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct BlazeConfig {
    /// JVM heap size, passed as `-Xmx`. Unset leaves the JVM default of 25% of the host memory.
    pub heap_size: Option<String>,
    /// RocksDB block cache in MiB. Lives outside the JVM heap, so it adds to `heap_size`.
    #[serde(default = "default_block_cache_size")]
    pub block_cache_size: u32,
    /// CQL expression cache in MiB. Part of the JVM heap. 0 disables the cache.
    #[serde(default = "default_cql_expr_cache_size")]
    pub cql_expr_cache_size: u32,
    /// Fraction of the JVM heap used for the resource cache. Unset leaves Blaze's default of 0.25.
    pub resource_cache_size_ratio: Option<f32>,
}

fn default_block_cache_size() -> u32 {
    1024
}

fn default_cql_expr_cache_size() -> u32 {
    128
}

impl Default for BlazeConfig {
    fn default() -> Self {
        Self {
            heap_size: None,
            block_cache_size: default_block_cache_size(),
            cql_expr_cache_size: default_cql_expr_cache_size(),
            resource_cache_size_ratio: None,
        }
    }
}

#[derive(Debug, Template)]
#[template(path = "blaze.yml")]
pub struct Blaze<T>
where
    Self: Service,
{
    r#for: PhantomData<T>,
    traefik_conf: Option<BlazeTraefikConfig>,
    tuning: BlazeConfig,
}

impl<T> Blaze<T>
where
    Self: Service,
{
    pub fn get_url() -> Url {
        Url::from_str(&format!("http://{}:8080", Self::service_name())).unwrap()
    }
}

impl<T: BlazeProvider> Service for Blaze<T> {
    type Dependencies = (Traefik,);
    type ServiceConfig = &'static Config;

    fn from_config(conf: Self::ServiceConfig, (traefik,): super::Deps<Self>) -> Self {
        let traefik_conf = T::treafik_exposure();
        if let Some(conf) = &traefik_conf {
            traefik.add_basic_auth_user(conf.middleware_and_user_name.clone())
        }
        Self {
            r#for: PhantomData,
            traefik_conf,
            tuning: T::blaze_config(conf).cloned().unwrap_or_default(),
        }
    }

    fn service_name() -> String {
        T::balze_service_name()
    }
}

pub trait BlazeProvider: 'static {
    fn balze_service_name() -> String;

    /// relative path where this balze should be exposed through traefik. Defaults to None
    fn treafik_exposure() -> Option<BlazeTraefikConfig> {
        None
    }

    /// Tuning for this project's Blaze. Defaults to [`BlazeConfig::default`] when absent.
    fn blaze_config(_conf: &'static Config) -> Option<&'static BlazeConfig> {
        None
    }
}

#[derive(Debug)]
pub struct BlazeTraefikConfig {
    pub path: String,
    pub middleware_and_user_name: String,
}

impl<T: Service> BlazeProvider for T {
    fn balze_service_name() -> String {
        format!("{}-blaze", <T as Service>::service_name())
    }
}
