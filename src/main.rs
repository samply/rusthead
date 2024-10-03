//! Options:
//! 
//! 1:
//! Express the whole config in terms of services which deserialize based on the toml config.
//! In this version a service would be very specific like beam-proxy-ccp with a lot of defaults.
//! 
//! 2:
//! A service is an abstraction around a container and the Config is a hardcoded struct which gets deserialized and based on that we create these services programatically

use dep_map::DepMap;
use serde::{Deserialize, Serialize};
use url::Url;
mod services;
mod dep_map;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
struct Config {
    project: Project,
    http_proxy_url: Option<Url>,
    https_proxy_url: Option<Url>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Project {
    Ccp,
    Bbmri,
    Minimal
}


fn main() -> anyhow::Result<()> {
    let conf_path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "/etc/bridgehead/conf.toml".into());
    let conf: Config = toml::from_str(&std::fs::read_to_string(conf_path)?)?;

    let mut dep_map = DepMap::default();
    match conf.project {
        Project::Ccp => {

        },
        Project::Bbmri => todo!(),
        Project::Minimal => todo!(),
    }
    Ok(())
}
