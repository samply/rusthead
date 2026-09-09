use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Output},
};

fn script(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\nset -eu\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

struct Installation {
    temp: tempfile::TempDir,
}

impl Installation {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir(root.join("bin")).unwrap();
        fs::create_dir(root.join("site with spaces")).unwrap();
        fs::write(
            root.join("site with spaces/custom.toml"),
            "site_id = 'test'\nhostname = 'localhost'\n[ccp]\n",
        )
        .unwrap();
        script(&root.join("bin/id"), "test -f \"$INSTALL_TEST_ROOT/user\"");
        script(
            &root.join("bin/useradd"),
            "echo useradd >> \"$INSTALL_TEST_ROOT/log\"\ntouch \"$INSTALL_TEST_ROOT/user\"",
        );
        script(&root.join("bin/usermod"), ":");
        script(&root.join("bin/chown"), ":");
        script(&root.join("bin/systemctl"), "exit 3");
        script(
            &root.join("bin/sudo"),
            r#"
shift 2
if [ "$1" = git ]; then exec "$@"; fi
printf '%s\n' "$@" >> "$INSTALL_TEST_ROOT/log"
exit "${INSTALL_TEST_UPDATE_STATUS:-0}"
"#,
        );
        script(
            &root.join("bin/docker"),
            r#"
echo enroll >> "$INSTALL_TEST_ROOT/log"
while [ "$1" != --output-file ]; do shift; done
key="$2"
shift 2
while [ "$1" != --proxy-id ]; do shift; done
echo "$2" >> "$INSTALL_TEST_ROOT/proxies"
if [ -f "$INSTALL_TEST_ROOT/fail-network" ] && [ "$2" = "$(cat "$INSTALL_TEST_ROOT/fail-network")" ]; then exit 9; fi
if [ ! -f "$key" ]; then printf 'private key\n' > "$key"; fi
"#,
        );
        Self { temp }
    }

    fn run(&self, update_status: u8) -> Output {
        self.run_command("install", update_status)
    }

    fn run_command(&self, command: &str, update_status: u8) -> Output {
        let managed = self
            .temp
            .path()
            .join("site with spaces/.rusthead/bin/rusthead");
        let executable = if managed.exists() {
            managed
        } else {
            env!("CARGO_BIN_EXE_rusthead").into()
        };
        Command::new(executable)
            .args(["--config", "site with spaces/custom.toml", command])
            .current_dir(self.temp.path())
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.temp.path().join("bin").display(),
                    std::env::var("PATH").unwrap()
                ),
            )
            .env("INSTALL_TEST_ROOT", self.temp.path())
            .env("INSTALL_TEST_UPDATE_STATUS", update_status.to_string())
            .env(
                "GIT_CONFIG_GLOBAL",
                self.temp.path().join("global.gitconfig"),
            )
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap()
    }

    fn enrolled_networks(&self) -> Vec<serde_json::Value> {
        let value: serde_json::Value = serde_json::from_slice(
            &fs::read(
                self.temp
                    .path()
                    .join("site with spaces/.rusthead/enrollment.json"),
            )
            .unwrap(),
        )
        .unwrap();
        value["enrolled_beam_networks"].as_array().unwrap().clone()
    }

    fn local_conf(&self) -> toml::Table {
        toml::from_str(
            &fs::read_to_string(self.temp.path().join("site with spaces/config.local.toml"))
                .unwrap(),
        )
        .unwrap()
    }
}

#[test]
#[ignore = "requires root; run with unshare --user --map-root-user cargo test --test install -- --ignored"]
fn repeated_install_preserves_repository_and_private_key() {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let installation = Installation::new();
    for _ in 0..2 {
        let result = installation.run(3);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let root = installation.temp.path();
    let log = fs::read_to_string(root.join("log")).unwrap();
    assert_eq!(log.lines().filter(|line| *line == "useradd").count(), 1);
    assert_eq!(log.lines().filter(|line| *line == "enroll").count(), 1);
    assert!(log.contains(root.join("site with spaces/custom.toml").to_str().unwrap()));
    let managed = root.join("site with spaces/.rusthead/bin/rusthead");
    assert!(managed.is_file());
    assert!(log.contains(managed.to_str().unwrap()));
    assert_ne!(
        fs::metadata(managed).unwrap().permissions().mode() & 0o111,
        0
    );
    let key = root.join("site with spaces/pki/test.priv.pem");
    assert_eq!(fs::read_to_string(&key).unwrap(), "private key\n");
    assert_eq!(
        fs::metadata(key).unwrap().permissions().mode() & 0o7777,
        0o600
    );
    let global = fs::read_to_string(root.join("global.gitconfig")).unwrap();
    assert_eq!(global.matches("directory =").count(), 1);
    let repo = root.join("site with spaces");
    let git_config = fs::read_to_string(repo.join(".git/config")).unwrap();
    assert!(git_config.contains("sharedrepository = 1"));
    assert_eq!(
        git_config.matches("email = bridgehead@samply.de").count(),
        1
    );
    assert_eq!(
        fs::read_to_string(repo.join(".git/HEAD")).unwrap(),
        "ref: refs/heads/main\n"
    );
}

#[test]
#[ignore = "requires root; run with unshare --user --map-root-user cargo test --test install -- --ignored"]
fn failed_update_stops_installation_before_enrollment() {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let installation = Installation::new();
    let result = installation.run(7);
    assert_eq!(
        result.status.code(),
        Some(7),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        !installation
            .temp
            .path()
            .join("site with spaces/pki/test.priv.pem")
            .exists()
    );
}

#[test]
fn install_requires_root() {
    use std::os::unix::process::CommandExt;
    let installation = Installation::new();
    // --config must remain accessible while running as an unprivileged account.
    fs::set_permissions(installation.temp.path(), fs::Permissions::from_mode(0o755)).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_rusthead"));
    command
        .arg("--config")
        .arg(
            installation
                .temp
                .path()
                .join("site with spaces/custom.toml"),
        )
        .arg("install");
    if unsafe { libc::geteuid() } == 0 {
        command.uid(65534);
    }
    let output = command.output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must be run as root"));
}

#[test]
#[ignore = "requires root; run with unshare --user --map-root-user cargo test --test install -- --ignored"]
fn new_network_is_enrolled_without_repeating_existing_enrollment() {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let installation = Installation::new();
    assert!(installation.run(0).status.success());
    let root = installation.temp.path();
    let site = root.join("site with spaces");
    let before = installation.local_conf();
    fs::write(
        site.join("custom.toml"),
        "site_id = 'test'\nhostname = 'localhost'\n[ccp]\n[bbmri]\n",
    )
    .unwrap();
    fs::write(site.join(".env"), "KEEP=unchanged\n").unwrap();
    let result = installation.run_command("enroll", 0);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let local = installation.local_conf();
    assert!(!local.contains_key("beam_networks"));
    assert!(!local.contains_key("enrolled_beam_networks"));
    assert_eq!(installation.enrolled_networks().len(), 2);
    assert_eq!(local["seed"], before["seed"]);
    assert_eq!(
        fs::read_to_string(site.join(".env")).unwrap(),
        "KEEP=unchanged\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("proxies"))
            .unwrap()
            .lines()
            .count(),
        2
    );
    assert!(installation.run_command("enroll", 0).status.success());
    assert_eq!(
        fs::read_to_string(root.join("proxies"))
            .unwrap()
            .lines()
            .count(),
        2
    );
}

#[test]
#[ignore = "requires root; run with unshare --user --map-root-user cargo test --test install -- --ignored"]
fn partial_enrollment_is_persisted_and_only_failed_networks_are_retried() {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let installation = Installation::new();
    let root = installation.temp.path();
    fs::write(
        root.join("site with spaces/custom.toml"),
        "site_id = 'test'\nhostname = 'localhost'\n[ccp]\n[bbmri]\n",
    )
    .unwrap();
    fs::write(root.join("fail-network"), "test.broker.ccp-it.dktk.dkfz.de").unwrap();
    assert!(!installation.run(0).status.success());
    let local = installation.local_conf();
    assert!(!local.contains_key("beam_networks"));
    assert!(!local.contains_key("enrolled_beam_networks"));
    assert_eq!(installation.enrolled_networks().len(), 1);
    fs::remove_file(root.join("fail-network")).unwrap();
    assert!(installation.run(0).status.success());
    let proxies = fs::read_to_string(root.join("proxies")).unwrap();
    assert_eq!(
        proxies
            .lines()
            .filter(|line| *line == "test.broker.bbmri.samply.de")
            .count(),
        1
    );
    assert_eq!(
        proxies
            .lines()
            .filter(|line| *line == "test.broker.ccp-it.dktk.dkfz.de")
            .count(),
        2
    );
    assert_eq!(installation.enrolled_networks().len(), 2);
}
