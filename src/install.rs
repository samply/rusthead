use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::{Context, bail, ensure};
use duct::cmd;

use crate::{config::Config, modules, services::ServiceMap, update_state::UpdateLock};

fn require_root() -> anyhow::Result<()> {
    ensure!(
        unsafe { libc::geteuid() } == 0,
        "This command must be run as root."
    );
    Ok(())
}

fn load_materialized(config: &PathBuf) -> anyhow::Result<&'static Config> {
    let conf = Box::leak(Box::new(Config::load(config)?));
    let mut services = ServiceMap::new(conf);
    for module in modules::MODULES {
        services.install_module(*module);
    }
    services.materialize();
    Ok(conf)
}

pub fn install(config: &PathBuf) -> anyhow::Result<ExitCode> {
    require_root()?;
    let conf = load_materialized(config)?;
    // Persist the seed and pending networks before update runs in another process.
    conf.save_local_conf()?;
    let executable = std::env::current_exe().context("Failed to locate rusthead executable")?;
    if cmd!("id", "-u", "bridgehead")
        .stdout_null()
        .stderr_null()
        .unchecked()
        .run()?
        .status
        .success()
    {
        println!("Using existing user bridgehead.");
        // Also repair membership if the existing account has a different primary group.
        cmd!("usermod", "-a", "-G", "docker", "bridgehead")
            .run()
            .context("Failed to ensure bridgehead belongs to the docker group")?;
    } else {
        cmd!("useradd", "-M", "-g", "docker", "-N", "bridgehead")
            .run()
            .context("Failed to create bridgehead user (the docker group must exist)")?;
    }
    cmd!("chown", "-R", "-h", "bridgehead:docker", &conf.path)
        .run()
        .context("Failed to set installation ownership")?;
    share_permissions(&conf.path, &private_key(conf))?;
    cmd!(
        "sudo",
        "-u",
        "bridgehead",
        "git",
        "init",
        "-b",
        "main",
        "--shared=group"
    )
    .dir(&conf.path)
    .run()
    .context("Failed to initialize shared Git repository")?;
    configure_git(conf)?;

    let systemd = match cmd!("systemctl", "status", "docker")
        .stdout_null()
        .stderr_null()
        .unchecked()
        .run()
    {
        Ok(output) => output.status.success(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error).context("Failed to check Docker systemd service"),
    };
    if systemd {
        install_systemd(Path::new("/etc/systemd/system"), &executable, config)?;
        cmd!("systemctl", "daemon-reload").run()?;
        cmd!("systemctl", "enable", "bridgehead.service").run()?;
    } else {
        println!(
            "Systemd is not active or docker is not running via systemd. Skipping systemd setup."
        );
    }
    let status = run_update(&executable, config, &conf.path)?;
    if !matches!(status, 0 | 3) {
        return Ok(ExitCode::from(status));
    }
    // Update may have changed local credentials; do not overwrite them with our earlier copy.
    let needs_enrollment = {
        let _lock = UpdateLock::acquire(&conf.path)?;
        let conf = load_materialized(config)?;
        let pending = !conf.pending_beam_networks().is_empty();
        enroll_pending_networks(conf)?;
        pending
    };
    if needs_enrollment {
        // Accept enrollment's local inputs before the clean-only timer starts.
        let status = run_update(&executable, config, &conf.path)?;
        if !matches!(status, 0 | 3) {
            return Ok(ExitCode::from(status));
        }
    }
    if systemd {
        cmd!("systemctl", "enable", "--now", "bridgehead-update.timer").run()?;
    }
    println!("Installation complete.");
    println!(
        "Start with 'systemctl start bridgehead' or 'rusthead --config {} compose up'.",
        config.display()
    );
    Ok(ExitCode::SUCCESS)
}

fn run_update(executable: &Path, config: &Path, directory: &Path) -> anyhow::Result<u8> {
    let status = cmd!(
        "sudo",
        "-u",
        "bridgehead",
        executable,
        "--config",
        config,
        "update",
        "commit"
    )
    .dir(directory)
    .unchecked()
    .run()
    .context("Failed to run bridgehead update")?
    .status;
    match status.code() {
        Some(code) => {
            if !matches!(code, 0 | 3) {
                eprintln!("Failed to update bridgehead");
            }
            Ok(u8::try_from(code)?)
        }
        None => bail!("Bridgehead update was killed by a signal"),
    }
}

fn private_key(conf: &Config) -> PathBuf {
    conf.path.join(format!("pki/{}.priv.pem", conf.site_id))
}

// Do not follow symlinks out of the installation or make enrollment keys group writable.
fn share_permissions(path: &Path, key: &Path) -> anyhow::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Ok(());
    }
    let mode = if path == key {
        0o600
    } else {
        metadata.permissions().mode() | 0o2020
    };
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .with_context(|| format!("Failed to set permissions on {}", path.display()))?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            share_permissions(&entry?.path(), key)?;
        }
    }
    Ok(())
}

fn configure_git(conf: &Config) -> anyhow::Result<()> {
    // --add in the old installer appended another entry on every invocation.
    let trusted = cmd!("git", "config", "--global", "--get-all", "safe.directory")
        .stdout_capture()
        .unchecked()
        .run()?;
    ensure!(
        matches!(trusted.status.code(), Some(0 | 1)),
        "Failed to read Git safe.directory configuration"
    );
    if !String::from_utf8(trusted.stdout)?
        .lines()
        .any(|line| line == conf.path.to_string_lossy())
    {
        cmd!(
            "git",
            "config",
            "--global",
            "--add",
            "safe.directory",
            &conf.path
        )
        .run()?;
    }
    for (key, value) in [
        ("user.email", "bridgehead@samply.de"),
        ("user.name", "Bridgehead"),
    ] {
        if !cmd!("git", "config", "--get", key)
            .dir(&conf.path)
            .stdout_null()
            .unchecked()
            .run()?
            .status
            .success()
        {
            cmd!("git", "config", "--local", key, value)
                .dir(&conf.path)
                .run()?;
        }
    }
    for key in ["http.proxy", "https.proxy"] {
        if let Some(proxy) = &conf.https_proxy_url {
            cmd!(
                "git",
                "config",
                "--local",
                "--replace-all",
                key,
                proxy.as_str()
            )
            .dir(&conf.path)
            .run()?;
        } else {
            let status = cmd!("git", "config", "--local", "--unset-all", key)
                .dir(&conf.path)
                .unchecked()
                .run()?
                .status;
            ensure!(
                matches!(status.code(), Some(0 | 5)),
                "Failed to remove Git {key} configuration"
            );
        }
    }
    Ok(())
}

// systemd has its own quoting and specifier expansion, independent of shell quoting.
fn unit_arg(path: &Path) -> anyhow::Result<String> {
    let value = path.to_str().context("systemd paths must be UTF-8")?;
    ensure!(
        !value.chars().any(char::is_control),
        "systemd paths must not contain control characters"
    );
    Ok(format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
            .replace('$', "$$")
    ))
}

fn install_systemd(directory: &Path, executable: &Path, config: &Path) -> anyhow::Result<()> {
    let command = format!("{} --config {}", unit_arg(executable)?, unit_arg(config)?);
    let units = [
        ("bridgehead.service", format!("[Unit]\nDescription=Bridgehead Service\nRequires=docker.service\n\n[Service]\nExecStart={command} compose up --abort-on-container-exit\nRestart=always\nUser=bridgehead\nGroup=docker\n\n[Install]\nWantedBy=multi-user.target\n")),
        ("bridgehead-update.service", format!("[Unit]\nDescription=Bridgehead Update Service\nRequires=docker.service\n\n[Service]\nExecStart={command} update sync\nSuccessExitStatus=3\nUser=bridgehead\nGroup=docker\nExecStopPost=+/bin/bash -c 'if [ \"$$EXIT_STATUS\" = \"3\" ] || [ \"$$EXIT_STATUS\" = \"4\" ]; then systemctl restart bridgehead.service; fi'\n")),
        ("bridgehead-update.timer", "[Unit]\nDescription=Daily Updates at 6am of Bridgehead\n\n[Timer]\nOnCalendar=*-*-* 06:00:00\nPersistent=true\n\n[Install]\nWantedBy=basic.target\n".into()),
    ];
    fs::create_dir_all(directory)?;
    for (name, contents) in units {
        let path = directory.join(name);
        if fs::read(&path).ok().as_deref() != Some(contents.as_bytes()) {
            fs::write(&path, contents)
                .with_context(|| format!("Failed to write {}", path.display()))?;
        }
    }
    Ok(())
}

pub fn enroll(config: &PathBuf) -> anyhow::Result<ExitCode> {
    require_root()?;
    let root = if config.is_dir() {
        config.as_path()
    } else {
        config.parent().context("Configuration has no parent")?
    };
    let _lock = UpdateLock::acquire(root)?;
    let conf = load_materialized(config)?;
    enroll_pending_networks(conf)?;
    Ok(ExitCode::SUCCESS)
}

fn enroll_pending_networks(conf: &Config) -> anyhow::Result<()> {
    conf.save_local_conf()?;
    // Persist invalidation of old receipts when the site's private key is missing.
    conf.enrollment.borrow().save(&conf.path)?;
    let networks = conf.pending_beam_networks();
    if networks.is_empty() {
        println!("No Beam networks pending enrollment.");
        return Ok(());
    }
    let key = private_key(conf);
    for broker in &networks {
        println!("Enrolling {}.{broker}", conf.site_id);
        cmd!(
            "docker",
            "run",
            "--rm",
            "-v",
            format!("{0}:{0}", conf.path.join("pki").display()),
            "docker.verbis.dkfz.de/cache/samply/beam-enroll:latest",
            "--output-file",
            &key,
            "--proxy-id",
            format!("{}.{broker}", conf.site_id)
        )
        .run()
        .context("Beam enrollment failed")?;
        fs::set_permissions(&key, fs::Permissions::from_mode(0o600))?;
        cmd!("chown", "bridgehead:docker", &key).run()?;
        {
            let mut enrollment = conf.enrollment.borrow_mut();
            enrollment.enrolled_beam_networks.insert(broker.clone());
        }
        // Save each success independently without modifying configuration or the update baseline.
        conf.enrollment.borrow().save(&conf.path)?;
    }
    if !networks.is_empty() {
        println!(
            "After getting the CSRs enrolled you may start the bridgehead service with 'systemctl start bridgehead'."
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_state_survives_reload_and_tracks_configuration_changes() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config.toml");
        let configured = "site_id = 'test'\nhostname = 'localhost'\n[ccp]\n";
        fs::write(&config, configured).unwrap();
        // Older local configurations must load without losing their existing seed.
        fs::write(temp.path().join("config.local.toml"), "seed = 42\n").unwrap();
        let conf = load_materialized(&config).unwrap();
        let networks = conf.beam_networks.borrow().clone();
        assert_eq!(networks.len(), 1);
        assert_eq!(conf.pending_beam_networks(), networks);
        fs::write(private_key(conf), "key").unwrap();
        {
            let mut enrollment = conf.enrollment.borrow_mut();
            enrollment.enrolled_beam_networks = networks.clone();
        }
        conf.enrollment.borrow().save(&conf.path).unwrap();
        assert!(
            load_materialized(&config)
                .unwrap()
                .pending_beam_networks()
                .is_empty()
        );

        fs::write(&config, "site_id = 'test'\nhostname = 'localhost'\n").unwrap();
        let disabled = load_materialized(&config).unwrap();
        assert!(disabled.beam_networks.borrow().is_empty());
        assert_eq!(
            disabled.enrollment.borrow().enrolled_beam_networks,
            networks
        );
        disabled.save_local_conf().unwrap();
        fs::write(&config, configured).unwrap();
        assert!(
            load_materialized(&config)
                .unwrap()
                .pending_beam_networks()
                .is_empty()
        );

        fs::write(&config, configured.replace("'test'", "'new-site'")).unwrap();
        let renamed = load_materialized(&config).unwrap();
        fs::write(private_key(renamed), "another key").unwrap();
        assert!(
            load_materialized(&config)
                .unwrap()
                .pending_beam_networks()
                .is_empty()
        );
        fs::write(&config, configured).unwrap();
        fs::remove_file(private_key(conf)).unwrap();
        assert_eq!(
            load_materialized(&config).unwrap().pending_beam_networks(),
            networks
        );
    }

    #[test]
    fn units_are_stable_and_reference_the_rust_binary_and_selected_config() {
        let temp = tempfile::tempdir().unwrap();
        let executable = Path::new("/opt/bridge head/rusthead");
        let config = Path::new("/srv/bridge head/custom.toml");
        install_systemd(temp.path(), executable, config).unwrap();
        let service = temp.path().join("bridgehead.service");
        let modified = fs::metadata(&service).unwrap().modified().unwrap();
        install_systemd(temp.path(), executable, config).unwrap();
        assert_eq!(
            fs::metadata(&service).unwrap().modified().unwrap(),
            modified
        );
        assert!(fs::read_to_string(&service).unwrap().contains("ExecStart=\"/opt/bridge head/rusthead\" --config \"/srv/bridge head/custom.toml\" compose up --abort-on-container-exit"));
        assert!(
            fs::read_to_string(temp.path().join("bridgehead-update.service"))
                .unwrap()
                .contains("$$EXIT_STATUS")
        );
        let update_unit =
            fs::read_to_string(temp.path().join("bridgehead-update.service")).unwrap();
        assert!(update_unit.contains(" update sync\nSuccessExitStatus=3\n"));
        assert!(update_unit.contains("= \"4\""));
        assert!(
            fs::read_to_string(temp.path().join("bridgehead-update.timer"))
                .unwrap()
                .contains("OnCalendar=*-*-* 06:00:00")
        );
        assert_eq!(
            unit_arg(Path::new("/srv/50%/$site")).unwrap(),
            "\"/srv/50%%/$$site\""
        );
        assert!(unit_arg(Path::new("/srv/line\nbreak")).is_err());
    }

    #[test]
    fn sharing_permissions_preserves_keys_and_does_not_follow_symlinks() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("site");
        fs::create_dir(&root).unwrap();
        let key = root.join("key.pem");
        fs::write(&key, "key").unwrap();
        let external = temp.path().join("external");
        fs::write(&external, "external").unwrap();
        fs::set_permissions(&external, fs::Permissions::from_mode(0o600)).unwrap();
        symlink(&external, root.join("link")).unwrap();
        for _ in 0..2 {
            share_permissions(&root, &key).unwrap();
        }
        assert_eq!(
            fs::metadata(&key).unwrap().permissions().mode() & 0o7777,
            0o600
        );
        assert_eq!(
            fs::metadata(&external).unwrap().permissions().mode() & 0o7777,
            0o600
        );
        assert_eq!(
            fs::metadata(&root).unwrap().permissions().mode() & 0o2020,
            0o2020
        );
    }
}
