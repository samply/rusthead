use askama::Template;
use serde::Deserialize;

use crate::{config::Config, services::Service};

#[derive(Debug, Clone, Deserialize)]
pub struct DnpmNodeConf {
    pub zpm_site: String,
    synth_num: Option<i32>,
}

#[derive(Debug, Template)]
#[template(path = "dnpm_node.yml")]
pub struct DnpmNode {
    conf: DnpmNodeConf,
    host: String,
    site_id: String,
    authup_secret: String,
    mysql_root_password: String,
}

impl Service for DnpmNode {
    type Dependencies = ();

    type ServiceConfig = (DnpmNodeConf, &'static Config);

    fn from_config((conf, global_conf): Self::ServiceConfig, (): super::Deps<Self>) -> Self {
        let mut local_conf = global_conf.local_conf.borrow_mut();
        Self {
            conf,
            host: global_conf.hostname.to_string(),
            site_id: global_conf.site_id.to_string(),
            authup_secret: local_conf.generate_secret::<10, Self>("authup"),
            mysql_root_password: local_conf.generate_secret::<10, Self>("mysql_root"),
        }
    }

    fn service_name() -> String {
        "dnpm-node".to_string()
    }
}
