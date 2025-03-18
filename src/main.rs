use anyhow::Context;
use config::Config;
use services::ServiceMap;

mod config;
mod modules;
mod services;
mod utils;

fn main() -> anyhow::Result<()> {
    let conf = Config::load().context("Failed to load config")?;
    let mut services = ServiceMap::default();
    modules::MODULES
        .iter()
        .for_each(|&m| services.install_module(m, &conf));
    services.write_composables(&conf.srv_dir).context("Failed to write services")?;
    Ok(())
}
