use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::Context;
use clap::Parser;
use config::Config;
use duct::cmd;
use services::ServiceMap;

mod config;
mod git;
mod install;
mod modules;
mod services;
mod utils;

#[derive(Debug, clap::Subcommand)]
enum Subcommand {
    Compose {
        #[clap(trailing_var_arg = true)]
        compose_args: Vec<String>,
    },
    Update,
    Bootstrap,
    Enroll,
    Install,
}

#[derive(Debug, clap::Parser)]
struct Args {
    #[clap(
        short,
        long,
        env = "BRIDGEHEAD_CONFIG_PATH",
        default_value = "./config.toml"
    )]
    /// Path to the bridgehead configuration file
    config: PathBuf,

    #[clap(subcommand)]
    command: Subcommand,
}

fn main() -> anyhow::Result<ExitCode> {
    let args = Args::parse();
    let config = args
        .config
        .canonicalize()
        .context("Failed to resolve config path")?;
    let config_dir = if config.is_dir() {
        config.as_path()
    } else {
        config.parent().unwrap()
    };
    std::env::set_current_dir(config_dir)?;
    let cwd = std::env::current_dir()?;
    match &args.command {
        Subcommand::Compose { compose_args } => compose_command(&cwd, compose_args)?
            .run()
            .context("Failed to run docker compose")?
            .status
            .code()
            .and_then(|c| Some(ExitCode::from(u8::try_from(c).ok()?)))
            .ok_or(anyhow::anyhow!("Killed by signal")),
        Subcommand::Update => update(&config),
        Subcommand::Bootstrap => todo!("Not implemented"),
        Subcommand::Enroll => install::enroll(&config),
        Subcommand::Install => install::install(&config),
    }
}

fn compose_command(config_dir: &Path, compose_args: &[String]) -> anyhow::Result<duct::Expression> {
    let mut services = config_dir
        .join("services")
        .read_dir()?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    services.sort();
    let mut args = vec!["compose".into(), "-p".into(), "bridgehead".into()];
    for service in services {
        args.push(std::ffi::OsString::from("-f"));
        args.push(service.into_os_string());
    }
    args.extend(compose_args.iter().map(std::ffi::OsString::from));
    Ok(cmd("docker", args).dir(config_dir))
}

pub fn update(config: &PathBuf) -> anyhow::Result<ExitCode> {
    let conf =
        Config::load(config).with_context(|| format!("Failed to load config from {config:?}"))?;
    let conf: &'static Config = Box::leak(Box::new(conf));
    let mut services = ServiceMap::new(conf);
    modules::MODULES
        .iter()
        .for_each(|&m| services.install_module(m));
    services.write_all()?;
    Ok(ExitCode::SUCCESS)
}
