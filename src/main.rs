use std::path::PathBuf;

use anyhow::Context;
use bridgehead::Bridgehead;
use clap::Parser;
use config::Config;
use services::ServiceMap;

mod bridgehead;
mod config;
mod modules;
mod services;
mod utils;

#[derive(Debug, clap::Parser)]
enum Args {
    Bootstrap,
    Update {
        #[clap(
            short,
            long,
            env = "BRIDGEHEAD_CONFIG_PATH",
            default_value = "/etc/bridgehead"
        )]
        config: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let conf_path = match Args::parse() {
        Args::Bootstrap => {
            println!("{}", include_str!("../static/bootstrap.sh"));
            return Ok(());
        },
        Args::Update { config } => config,
    };
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
