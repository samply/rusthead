use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::Context;
use clap::Parser;
use config::Config;
use duct::cmd;

mod config;
mod enrollment;
mod git;
mod install;
mod modules;
mod self_update;
mod services;
mod update;
mod update_state;
mod utils;

#[derive(Debug, clap::Subcommand)]
enum Subcommand {
    Compose {
        #[clap(trailing_var_arg = true)]
        compose_args: Vec<String>,
    },
    /// Generate and record an installation update.
    Update {
        #[clap(subcommand)]
        mode: update::Mode,
    },
    /// Print a Bash script that installs the native binary and creates a configuration.
    Bootstrap,
    Enroll,
    Install {
        /// Skip systemd service and update timer setup.
        #[clap(long)]
        no_systemd: bool,
    },
}

#[derive(Debug, clap::Parser)]
#[clap(version)]
struct Args {
    #[clap(
        short,
        long,
        env = "BRIDGEHEAD_CONFIG_PATH",
        default_value = "./config.toml"
    )]
    /// Path to the bridgehead configuration file
    config: PathBuf,

    /// Keep the current executable when developing with a local build.
    #[clap(long, global = true)]
    no_self_update: bool,

    #[clap(subcommand)]
    command: Subcommand,
}

fn main() -> anyhow::Result<ExitCode> {
    let args = Args::parse();
    if let Subcommand::Bootstrap = args.command {
        println!("{}", include_str!("../static/bootstrap.sh"));
        return Ok(ExitCode::SUCCESS);
    }
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
        Subcommand::Compose { compose_args } => {
            update::warn_pending(&cwd);
            compose_command(&cwd, compose_args)?
                .unchecked()
                .run()
                .context("Failed to run docker compose")?
                .status
                .code()
                .and_then(|c| Some(ExitCode::from(u8::try_from(c).ok()?)))
                .ok_or(anyhow::anyhow!("Killed by signal"))
        }
        Subcommand::Update { mode } => update::run(&config, *mode, args.no_self_update),
        Subcommand::Bootstrap => unreachable!("bootstrap is handled before loading configuration"),
        Subcommand::Enroll => install::enroll(&config),
        Subcommand::Install { no_systemd } => {
            install::install(&config, args.no_self_update, *no_systemd)
        }
    }
}

fn compose_command(config_dir: &Path, compose_args: &[String]) -> anyhow::Result<duct::Expression> {
    compose_command_with_lock(config_dir, compose_args, true)
}

fn compose_command_with_lock(
    config_dir: &Path,
    compose_args: &[String],
    locked: bool,
) -> anyhow::Result<duct::Expression> {
    let mut services = config_dir
        .join("services")
        .read_dir()?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    services.retain(|p| {
        p.is_file() && matches!(p.extension().and_then(|s| s.to_str()), Some("yml" | "yaml"))
    });
    services.sort();
    for name in ["docker-compose.override.yml", "docker-image.lock.yml"] {
        if (locked || name != "docker-image.lock.yml") && config_dir.join(name).is_file() {
            services.push(config_dir.join(name));
        }
    }
    let mut args = vec![
        "compose".into(),
        "-p".into(),
        "bridgehead".into(),
        "--project-directory".into(),
        config_dir.as_os_str().to_owned(),
    ];
    if services.is_empty() {
        anyhow::bail!(
            "There are currently no services defined by your configuration.\nPlease enable a service in your config.toml and run `rusthead update commit`"
        );
    }
    for service in services {
        args.push(std::ffi::OsString::from("-f"));
        args.push(service.into_os_string());
    }
    args.extend([
        std::ffi::OsString::from("--env-file"),
        config_dir.join(".env").into_os_string(),
    ]);
    args.extend(compose_args.iter().map(std::ffi::OsString::from));
    Ok(cmd("docker", args).dir(config_dir))
}
