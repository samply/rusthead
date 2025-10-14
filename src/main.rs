use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use config::Config;
use services::ServiceMap;

mod bridgehead;
mod config;
mod git;
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
        }
        Args::Update { config } => config,
    };
    let conf = Config::load(&conf_path)
        .with_context(|| format!("Failed to load config from {conf_path:?}"))?;
    let conf: &'static _ = Box::leak(Box::new(conf));
    let mut services = ServiceMap::new(conf);
    modules::MODULES
        .iter()
        .for_each(|&m| services.install_module(m));
    let before_hashes = git::hash_untracked_files(conf)?;
    git::stash_if_dirty(conf)?;
    services.write_all()?;
    let _needs_restart = git::commit_all(conf, &before_hashes, &git::hash_untracked_files(conf)?)?;
    Ok(())
}
