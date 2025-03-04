use serde::{Deserialize, Serialize};
use services::{beam::DktkBroker, focus::Focus};
use url::Url;
mod dep_map;
mod modules;
mod services;
mod utils;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
struct Config {
    project: Project,
    site_id: String,
    http_proxy_url: Option<Url>,
    https_proxy_url: Option<Url>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Project {
    Ccp,
    Bbmri,
    Minimal,
}

fn main() -> anyhow::Result<()> {
    // let conf_path =
    //     std::env::var("CONFIG_PATH").unwrap_or_else(|_| "/etc/bridgehead/conf.toml".into());
    // let conf: Config = toml::from_str(&std::fs::read_to_string(conf_path)?)?;
    let conf = Config {
        project: Project::Ccp,
        http_proxy_url: None,
        https_proxy_url: None,
        site_id: "test".into(),
    };

    let mut services = dep_map::ServiceMap::default();
    match conf.project {
        Project::Ccp => {
            services.install::<Focus<DktkBroker>>(&conf);
            modules::CCP_MODULES
                .iter()
                .for_each(|&m| services.install_module(m, &conf));
            services.write_composables()?
        }
        Project::Bbmri => todo!(),
        Project::Minimal => todo!(),
    }
    Ok(())
}
