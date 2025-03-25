use std::path::PathBuf;

use anyhow::Context;
use bridgehead::Bridgehead;
use config::Config;
use services::ServiceMap;

mod bridgehead;
mod config;
mod modules;
mod services;
mod utils;

fn main() -> anyhow::Result<()> {
    let conf_path: PathBuf = std::env::var("BRIDGEHEAD_CONFIG_PATH")
        .unwrap_or_else(|_| "/etc/bridgehead".into())
        .into();
    let conf = Config::load(&conf_path)
        .with_context(|| format!("Failed to load config from {conf_path:?}"))?;
    let mut services = ServiceMap::default();
    modules::MODULES
        .iter()
        .for_each(|&m| services.install_module(m, &conf));
    services
        .write_composables(&conf.srv_dir)
        .context("Failed to write services")?;
    Bridgehead::new(&conf).write()?;
    conf.write_local_conf()?;
    Ok(())
}
