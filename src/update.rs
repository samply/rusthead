use crate::{
    Config,
    git::Repository,
    modules,
    services::ServiceMap,
    update_state::{self as state, OUTPUTS, State, UpdateLock},
};
use anyhow::{Context, ensure};
use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

#[derive(Debug, Clone, Copy, clap::Subcommand)]
pub enum Mode {
    /// Require clean inputs, pull upstream changes if enabled, and update generated files.
    Sync,
    /// Accept local input edits and commit them together with updated generated files.
    Commit,
}

fn ensure_volume_untracked(repo: &Repository, conf: &Config) -> anyhow::Result<()> {
    if let Some(volume) = state::volume_path(conf)?
        && let Ok(relative) = volume.strip_prefix(&repo.root)
    {
        ensure!(
            repo.run(&[
                "ls-files",
                "--",
                relative
                    .to_str()
                    .context("Volume directory must be UTF-8")?
            ])?
            .is_empty(),
            "Runtime data is tracked in Git; untrack the volume directory before updating"
        );
    }
    Ok(())
}

fn snapshot(root: &Path) -> anyhow::Result<State> {
    Ok(State {
        local_inputs: state::fingerprint(root, state::local_paths())?,
        outputs: state::fingerprint(root, OUTPUTS.iter().map(PathBuf::from))?,
    })
}

pub fn run(config: &PathBuf, mode: Mode, no_self_update: bool) -> anyhow::Result<ExitCode> {
    let config = if config.is_dir() {
        config.join("config.toml")
    } else {
        config.clone()
    };
    let root = config
        .parent()
        .context("Configuration has no parent directory")?;
    // Match the shared repository and installation permissions, including new generated files.
    unsafe { libc::umask(0o0002) };
    // Check repository boundaries before creating installation metadata.
    Repository::open(root)?;
    let _lock = UpdateLock::acquire(root)?;
    let repo = Repository::initialize(root)?;
    repo.ensure_idle()?;
    let initial = !repo.has_head()?;
    let baseline = State::load(root)?;
    let conf = Config::load(&config)?;
    let network = if conf.git_sync {
        Some(repo.upstream()?)
    } else {
        None
    };
    ensure_volume_untracked(&repo, &conf)?;
    let before = snapshot(root)?;
    if !initial {
        match mode {
            Mode::Sync => {
                ensure!(
                    !repo.dirty()?,
                    "Repository has pending changes; run update commit to accept them before update sync"
                );
                if let Some(ref baseline) = baseline {
                    ensure!(
                        before.local_inputs == baseline.local_inputs
                            && before.outputs.get(".env") == baseline.outputs.get(".env"),
                        "Local inputs or generated files changed; run update commit to accept input edits"
                    );
                }
            }
            Mode::Commit => {
                ensure!(
                    !repo.generated_dirty()? || state::pending_matches(root, &before.outputs)?,
                    "Generated files were edited; move customization into config or docker-compose.override.yml and restore generated files before updating"
                );
                if let Some(ref baseline) = baseline {
                    // Git handles tracked outputs; .env is ignored and must be checked separately.
                    ensure!(
                        before.outputs.get(".env") == baseline.outputs.get(".env")
                            || state::pending_matches(root, &before.outputs)?,
                        "Generated .env was edited; move changes into config.local.toml and restore .env before updating"
                    );
                }
            }
        }
    }
    if matches!(mode, Mode::Sync)
        && let Some(ref upstream) = network
    {
        repo.sync(upstream)?;
        repo.ensure_idle()?;
    }
    // Pulling may have changed both the configuration and its local path settings.
    let conf = Box::leak(Box::new(Config::load(&config)?));
    if !no_self_update && let Some(replacement) = crate::self_update::check(conf)? {
        drop(_lock);
        return replacement.resume(&config, mode);
    }
    ensure_volume_untracked(&repo, conf)?;
    state::ensure_ignore(conf)?;
    let mut services = ServiceMap::new(conf);
    for module in modules::MODULES {
        services.install_module(*module);
    }
    services.write_all().context("Generation failed; no update commit was created. Inspect any partial generated files before retrying")?;
    state::record_pending(root)?;
    services.generate_lockfile_and_pull(|| state::record_pending(root).map(|_| ()))
        .context("Image update failed; no update commit was created. Run update commit to retry; generated files must remain unedited")?;
    let after = snapshot(root)?;
    let changed = baseline.as_ref().is_none_or(|s| s.runtime_changed(&after));
    repo.commit()?;
    after.save(root)?;
    std::fs::remove_file(root.join(".rusthead/pending.json"))?;
    let push_failed = if let Some(ref upstream) = network {
        match repo.push(upstream) {
            Ok(()) => false,
            Err(error) => {
                eprintln!("Local update committed, but synchronization failed: {error:#}");
                true
            }
        }
    } else {
        false
    };
    if changed {
        println!("Updated runtime configuration. Please restart the bridgehead.");
    }
    Ok(ExitCode::from(match (changed, push_failed) {
        (false, false) => 0,
        (true, false) => 3,
        (false, true) => 1,
        (true, true) => 4,
    }))
}

pub fn warn_pending(root: &Path) {
    let check = || -> anyhow::Result<bool> {
        let Some(repo) = Repository::open(root)? else {
            return Ok(true);
        };
        if repo.dirty()? {
            return Ok(true);
        }
        let Some(baseline) = State::load(root)? else {
            return Ok(true);
        };
        let now = snapshot(root)?;
        Ok(now != baseline)
    };
    match check() {
        Ok(false) => {}
        Ok(true) => eprintln!(
            "Warning: configuration has pending changes or no successful update baseline; run update commit."
        ),
        Err(e) => eprintln!("Warning: cannot check pending configuration changes: {e:#}"),
    }
}
