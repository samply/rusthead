use std::collections::HashMap;

use serde::Deserialize;
use url::Url;
mod dep_map;
mod modules;
mod services;
mod utils;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    site_id: String,
    http_proxy: Option<Url>,
    https_proxy: Option<Url>,
    ccp: Option<CcpConfig>,
    // TODO Actual structs
    bbmri: Option<HashMap<String, toml::Value>>,
    // etc..
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CcpConfig {
    a: String,
    c: Vec<String>,
    d: HashMap<String, String>,
}

fn main() -> anyhow::Result<()> {
    let conf_path =
        std::env::var("CONFIG_PATH").unwrap_or_else(|_| "/etc/bridgehead/conf.toml".into());
    let conf: Config = toml::from_str(&std::fs::read_to_string(conf_path)?)?;

    let mut services = dep_map::ServiceMap::default();
    modules::MODULES
        .iter()
        .for_each(|&m| services.install_module(m, &conf));
    services.write_composables()?;
    Ok(())
}
