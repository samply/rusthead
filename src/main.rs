use std::{path::PathBuf, process::ExitCode};

use anyhow::Context;
use clap::Parser;
use config::Config;
use services::ServiceMap;

use crate::{bridgehead::Bridgehead, git::DiffTrackerResult};

mod bridgehead;
mod config;
mod git;
mod modules;
mod services;
mod utils;

#[derive(Debug, clap::Subcommand)]
enum BootstrapHelper {
    Bridgehead {
        #[clap(short, long, env = "BRIDGEHEAD_CONFIG_PATH")]
        config: PathBuf,
    },
}

#[derive(Debug, clap::Parser)]
enum Args {
    Bootstrap {
        #[clap(subcommand)]
        helper: Option<BootstrapHelper>,
    },
    Update {
        #[clap(short, long, env = "BRIDGEHEAD_CONFIG_PATH")]
        config: PathBuf,
    },
}

fn main() -> anyhow::Result<ExitCode> {
    let conf_path = match Args::parse() {
        Args::Bootstrap { helper: None } => {
            println!("{}", include_str!("../static/bootstrap.sh"));
            return Ok(ExitCode::SUCCESS);
        }
        Args::Bootstrap {
            helper: Some(BootstrapHelper::Bridgehead { config }),
        } => {
            let conf = Config::load(&config)
                .with_context(|| format!("Failed to load config from {config:?}"))?;
            let conf: &'static Config = Box::leak(Box::new(conf));
            Bridgehead::new(&conf).write()?;
            return Ok(ExitCode::SUCCESS);
        }
        Args::Update { config } => config,
    };
    let conf = Config::load(&conf_path)
        .with_context(|| format!("Failed to load config from {conf_path:?}"))?;
    let conf: &'static mut Config = Box::leak(Box::new(conf));
    let diff_tracker = match git::DiffTracker::start(&conf)? {
        DiffTrackerResult::Success(tracker) => Some(tracker),
        // git pull updated the repo -> reload the config
        DiffTrackerResult::NeedsConfigReload => {
            println!("Reloading config...");
            *conf = Config::load(&conf_path).with_context(|| {
                format!("Failed to load config from {conf_path:?} after update")
            })?;
            let DiffTrackerResult::Success(dt) = git::DiffTracker::start(&conf)? else {
                anyhow::bail!("We just pulled so we should not need to reload the config again");
            };
            Some(dt)
        }
        DiffTrackerResult::NotAGitRepo => None,
    };
    let mut services = ServiceMap::new(conf);
    modules::MODULES
        .iter()
        .for_each(|&m| services.install_module(m));
    services.write_all()?;
    if let Some(diff_tracker) = diff_tracker {
        let needs_restart = diff_tracker.commit()?;
        if needs_restart {
            println!("Updated the bridgehead. Please restart");
            Ok(ExitCode::from(3))
        } else {
            Ok(ExitCode::SUCCESS)
        }
    } else {
        // Most likely a new installation
        Ok(ExitCode::SUCCESS)
    }
}
