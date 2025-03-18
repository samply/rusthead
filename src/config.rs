use std::{collections::HashMap, path::PathBuf};

use serde::Deserialize;
use url::Url;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub site_id: String,
    pub http_proxy: Option<Url>,
    pub https_proxy: Option<Url>,
    pub ccp: Option<CcpConfig>,
    // TODO Actual structs
    pub bbmri: Option<HashMap<String, toml::Value>>,
    // etc..
    #[serde(default = "default_srv_dir")]
    pub srv_dir: PathBuf,
    #[serde(skip)]
    pub path: PathBuf,
}

fn default_srv_dir() -> PathBuf {
    PathBuf::from("/srv/docker/bridgehead")
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let conf_path: PathBuf = std::env::var("BRIDGEHEAD_CONFIG_PATH")
            .unwrap_or_else(|_| "/etc/bridgehead".into())
            .into();
        let mut conf: Config =
            toml::from_str(&std::fs::read_to_string(conf_path.join("config.toml"))?)?;
        conf.path = conf_path;
        Ok(conf)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CcpConfig {
    // TODO
}
