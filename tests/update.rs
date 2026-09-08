use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Output,
};

struct Site {
    _temp: tempfile::TempDir,
    root: PathBuf,
    bin: PathBuf,
    executable: PathBuf,
}
impl Site {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("site with spaces");
        let bin = temp.path().join("bin");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&bin).unwrap();
        fs::copy(
            env!("CARGO_BIN_EXE_rusthead"),
            temp.path().join("image-rusthead"),
        )
        .unwrap();
        fs::write(
            root.join("custom.toml"),
            "site_id = 'test'\nhostname = 'localhost'\n[ccp]\n",
        )
        .unwrap();
        fs::write(bin.join("docker"), r#"#!/bin/sh
set -eu
printf '%s\n' "$*" >> "$TEST_SITE/docker.log"
case "$1" in
  pull) test ! -f "$TEST_SITE/fail-binary-pull"; exit 0 ;;
  image) printf 'linux/amd64 sha256:%s\n' "$(cat "$TEST_SITE/binary-id" 2>/dev/null || echo one)"; exit 0 ;;
  create) echo extraction-container; exit 0 ;;
  cp) test ! -f "$TEST_SITE/fail-copy"; cp "$TEST_SITE/image-rusthead" "$3"; exit 0 ;;
  rm) exit 0 ;;
esac
case " $* " in
  *' --lock-image-digests '*)
    test ! -f "$TEST_SITE/fail-resolve"
    printf 'services:\n  ccp-blaze:\n    image: test@sha256:%s\n' "$(cat "$TEST_SITE/digest" 2>/dev/null || echo one)"
    ;;
  *' pull '*)
    test ! -f "$TEST_SITE/fail-pull"
    if test -f "$TEST_SITE/edit-during-pull"; then echo '# concurrent edit' >> "$TEST_SITE/site with spaces/custom.toml"; fi
    ;;
esac
"#).unwrap();
        fs::set_permissions(bin.join("docker"), fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            _temp: temp,
            root,
            bin,
            executable: env!("CARGO_BIN_EXE_rusthead").into(),
        }
    }
    fn command(&self, args: &[&str]) -> duct::Expression {
        duct::cmd(&self.executable, args)
            .dir(&self.root)
            .env(
                "PATH",
                format!("{}:{}", self.bin.display(), std::env::var("PATH").unwrap()),
            )
            .env("TEST_SITE", self._temp.path())
            .env("GIT_CONFIG_GLOBAL", self._temp.path().join("gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .stdout_capture()
            .stderr_capture()
            .unchecked()
    }
    fn run(&self, mode: &str) -> Output {
        self.command(&["--config", "custom.toml", "update", mode])
            .run()
            .unwrap()
    }
    fn expect(&self, mode: &str, status: i32) -> Output {
        let out = self.run(mode);
        assert_eq!(
            out.status.code(),
            Some(status),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        out
    }
    fn git(&self, args: &[&str]) -> String {
        git(&self.root, args)
    }
    fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"])
    }
    fn append(&self, path: &str, text: &str) {
        let path = self.root.join(path);
        let mut contents = fs::read_to_string(&path).unwrap();
        contents.push_str(text);
        fs::write(path, contents).unwrap();
    }
    fn remote(&self) -> PathBuf {
        let remote = self._temp.path().join("remote.git");
        git(
            self._temp.path(),
            &["init", "--bare", remote.to_str().unwrap()],
        );
        self.git(&["remote", "add", "origin", remote.to_str().unwrap()]);
        self.git(&["push", "-u", "origin", "main"]);
        let config = fs::read_to_string(self.root.join("custom.toml")).unwrap();
        fs::write(
            self.root.join("custom.toml"),
            format!("git_sync = true\n{config}"),
        )
        .unwrap();
        self.expect("commit", 0);
        remote
    }
}
fn git(root: &Path, args: &[&str]) -> String {
    let out = duct::cmd("git", args)
        .dir(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
        .stdout_capture()
        .stderr_capture()
        .run()
        .unwrap();
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn initial_update_noop_and_manual_edits() {
    let site = Site::new();
    site.expect("sync", 3);
    let head = site.head();
    assert!(site.git(&["status", "--porcelain"]).is_empty());
    site.expect("sync", 0);
    assert_eq!(head, site.head());
    assert!(!site.git(&["ls-files"]).contains(".env"));
    site.append("custom.toml", "# edited\n");
    site.git(&["add", "custom.toml"]);
    site.append("custom.toml", "# also unstaged\n");
    fs::write(site.root.join("operator notes.txt"), "notes").unwrap();
    site.expect("sync", 1);
    site.expect("commit", 0);
    assert_ne!(head, site.head());
    assert!(
        site.git(&["show", "HEAD:custom.toml"])
            .contains("also unstaged")
    );
    assert!(site.git(&["ls-files"]).contains("operator notes.txt"));
    assert!(site.git(&["status", "--porcelain"]).is_empty());
}

#[test]
fn ignored_inputs_outputs_and_compose_warnings() {
    let site = Site::new();
    site.expect("commit", 3);
    let head = site.head();
    site.append("config.local.toml", "# edit\n");
    site.expect("sync", 1);
    site.expect("commit", 0);
    assert_eq!(head, site.head());
    fs::write(
        site.root.join("docker-compose.override.yml"),
        "services: {}\n",
    )
    .unwrap();
    site.expect("sync", 1);
    site.expect("commit", 3);
    site.append("services/ccp-blaze.yml", "# manual edit\n");
    let out = site.expect("commit", 1);
    assert!(String::from_utf8_lossy(&out.stderr).contains("Generated files were edited"));
    assert!(
        fs::read_to_string(site.root.join("services/ccp-blaze.yml"))
            .unwrap()
            .contains("manual edit")
    );
    let compose = site
        .command(&["--config", "custom.toml", "compose", "logs"])
        .run()
        .unwrap();
    assert!(compose.status.success());
    assert!(String::from_utf8_lossy(&compose.stderr).contains("Warning:"));
    site.git(&["restore", "services"]);
    site.append(".env", "MANUAL=value\n");
    site.expect("commit", 1);
    fs::write(site.root.join("custom.toml"), "invalid syntax").unwrap();
    assert!(
        site.command(&["--config", "custom.toml", "compose", "down"])
            .run()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn image_updates_are_pinned_and_failures_do_not_commit() {
    let site = Site::new();
    site.expect("commit", 3);
    fs::write(
        site.root.join("docker-compose.override.yml"),
        "services: {}\n",
    )
    .unwrap();
    site.expect("commit", 3);
    fs::write(site._temp.path().join("digest"), "two").unwrap();
    site.expect("sync", 3);
    let log = fs::read_to_string(site._temp.path().join("docker.log")).unwrap();
    let resolution = log
        .lines()
        .rev()
        .find(|line| line.contains("--lock-image-digests"))
        .unwrap();
    assert!(resolution.contains("docker-compose.override.yml"));
    assert!(!resolution.contains("docker-image.lock.yml"));
    let pull = log
        .lines()
        .rev()
        .find(|line| line.contains("pull"))
        .unwrap();
    assert!(pull.contains("docker-image.lock.yml"));
    assert!(pull.contains("--project-directory"));
    assert!(
        pull.find("docker-compose.override.yml").unwrap()
            < pull.find("docker-image.lock.yml").unwrap()
    );
    assert!(
        site.git(&["show", "HEAD:docker-image.lock.yml"])
            .contains("two")
    );
    let head = site.head();
    fs::write(site._temp.path().join("fail-pull"), "").unwrap();
    site.append("custom.toml", "# pending\n");
    site.expect("commit", 1);
    assert_eq!(head, site.head());
    assert!(
        fs::read_to_string(site.root.join("custom.toml"))
            .unwrap()
            .contains("pending")
    );
}

#[test]
fn sync_fast_forwards_pushes_and_stops_on_divergence() {
    let site = Site::new();
    site.expect("commit", 3);
    let remote = site.remote();
    let admin = site._temp.path().join("admin");
    git(
        site._temp.path(),
        &[
            "clone",
            "--branch",
            "main",
            remote.to_str().unwrap(),
            admin.to_str().unwrap(),
        ],
    );
    let config = fs::read_to_string(admin.join("custom.toml"))
        .unwrap()
        .replace("localhost", "new.example");
    fs::write(
        admin.join("custom.toml"),
        format!("{config}\n[ccp.blaze]\nheap_size = '8g'\n"),
    )
    .unwrap();
    git(&admin, &["add", "."]);
    git(&admin, &["commit", "-m", "remote config"]);
    git(&admin, &["push"]);
    site.expect("sync", 3);
    assert!(
        fs::read_to_string(site.root.join("custom.toml"))
            .unwrap()
            .contains("new.example")
    );
    assert_eq!(site.head(), git(&remote, &["rev-parse", "refs/heads/main"]));
    site.append("custom.toml", "# local\n");
    site.git(&["add", "."]);
    site.git(&["commit", "-m", "local"]);
    git(&admin, &["pull", "--ff-only"]);
    fs::write(admin.join("notes"), "remote").unwrap();
    git(&admin, &["add", "."]);
    git(&admin, &["commit", "-m", "remote"]);
    git(&admin, &["push"]);
    let before = site.head();
    let out = site.expect("sync", 1);
    assert!(String::from_utf8_lossy(&out.stderr).contains("diverged"));
    assert_eq!(before, site.head());
}

#[test]
fn failed_push_keeps_commit_and_next_sync_retries() {
    let site = Site::new();
    site.expect("commit", 3);
    let remote = site.remote();
    let hook = remote.join("hooks/pre-receive");
    fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(site._temp.path().join("digest"), "two").unwrap();
    let before = site.head();
    site.expect("sync", 4);
    assert_ne!(before, site.head());
    assert!(site.git(&["status", "--porcelain"]).is_empty());
    site.expect("sync", 1);
    fs::remove_file(hook).unwrap();
    let head = site.head();
    site.expect("sync", 0);
    assert_eq!(head, site.head());
    assert_eq!(head, git(&remote, &["rev-parse", "refs/heads/main"]));
}

#[test]
fn explicit_sync_opt_in_and_missing_upstream() {
    let site = Site::new();
    site.expect("commit", 3);
    site.git(&["remote", "add", "origin", "/nonexistent/remote"]);
    site.expect("sync", 0);
    let text = fs::read_to_string(site.root.join("custom.toml")).unwrap();
    fs::write(
        site.root.join("custom.toml"),
        format!("git_sync = true\n{text}"),
    )
    .unwrap();
    assert!(String::from_utf8_lossy(&site.expect("commit", 1).stderr).contains("upstream"));
}

#[test]
fn concurrent_input_edits_and_update_lock() {
    use std::os::fd::AsRawFd;
    let site = Site::new();
    site.expect("commit", 3);
    let file = fs::OpenOptions::new()
        .write(true)
        .open(site.root.join(".rusthead/update.lock"))
        .unwrap();
    assert_eq!(
        unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    assert!(String::from_utf8_lossy(&site.expect("sync", 1).stderr).contains("Another update"));
    drop(file);
    let head = site.head();
    fs::write(site._temp.path().join("edit-during-pull"), "").unwrap();
    site.expect("sync", 1);
    assert_eq!(head, site.head());
    assert!(
        fs::read_to_string(site.root.join("custom.toml"))
            .unwrap()
            .contains("concurrent edit")
    );
}

#[test]
fn worktrees_are_supported_and_parent_repositories_rejected() {
    let site = Site::new();
    site.expect("commit", 3);
    let worktree = site._temp.path().join("worktree");
    site.git(&["worktree", "add", "-b", "other", worktree.to_str().unwrap()]);
    let out = site
        .command(&[
            "--config",
            worktree.join("custom.toml").to_str().unwrap(),
            "update",
            "commit",
        ])
        .run()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let nested = site.root.join("nested");
    fs::create_dir(&nested).unwrap();
    fs::write(
        nested.join("custom.toml"),
        "site_id = 'x'\nhostname = 'localhost'\n",
    )
    .unwrap();
    let out = site
        .command(&["--config", "nested/custom.toml", "update", "commit"])
        .run()
        .unwrap();
    assert!(!out.status.success());
    assert!(!nested.join(".rusthead").exists());
}

#[test]
fn failed_generation_can_be_retried_without_accepting_manual_output_edits() {
    let site = Site::new();
    site.expect("commit", 3);
    let head = site.head();
    site.append("custom.toml", "\n[ccp.blaze]\nheap_size = '8g'\n");
    fs::write(site._temp.path().join("fail-pull"), "").unwrap();
    site.expect("commit", 1);
    assert_eq!(site.head(), head);
    let generated = fs::read(site.root.join("services/ccp-blaze.yml")).unwrap();
    site.append(
        "services/ccp-blaze.yml",
        "# manual change after failed update\n",
    );
    site.expect("commit", 1);
    fs::write(site.root.join("services/ccp-blaze.yml"), generated).unwrap();
    fs::remove_file(site._temp.path().join("fail-pull")).unwrap();
    site.expect("commit", 3);
    assert!(!site.root.join(".rusthead/pending.json").exists());
    site.expect("sync", 0);
}

#[test]
fn ignored_runtime_changes_restart_without_empty_commits() {
    let site = Site::new();
    site.expect("commit", 3);
    let head = site.head();
    fs::write(site.root.join("pki/runtime.pem"), "certificate").unwrap();
    site.expect("sync", 1);
    site.expect("commit", 3);
    assert_eq!(head, site.head());
    assert!(!site.git(&["ls-files"]).contains("runtime.pem"));
    site.expect("sync", 0);
    fs::remove_file(site.root.join("pki/runtime.pem")).unwrap();
    site.expect("sync", 1);
    site.expect("commit", 3);
    assert_eq!(head, site.head());
}

#[test]
fn custom_ignores_are_preserved_and_unfinished_git_operations_are_rejected() {
    let site = Site::new();
    fs::write(site.root.join(".gitignore"), "/operator-data/\n").unwrap();
    site.expect("commit", 3);
    let contents = fs::read_to_string(site.root.join(".gitignore")).unwrap();
    assert!(contents.starts_with("/operator-data/\n"));
    site.expect("sync", 0);
    assert_eq!(
        contents,
        fs::read_to_string(site.root.join(".gitignore")).unwrap()
    );
    fs::write(site.root.join(".git/MERGE_HEAD"), site.head()).unwrap();
    assert!(String::from_utf8_lossy(&site.expect("sync", 1).stderr).contains("Git operation"));
    assert!(String::from_utf8_lossy(&site.expect("commit", 1).stderr).contains("Git operation"));
}

#[test]
fn bare_update_requires_a_mode_and_empty_installations_are_supported() {
    let site = Site::new();
    let help = site.command(&["update"]).run().unwrap();
    assert_eq!(help.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&help.stderr).contains("sync"));
    assert!(!site.root.join(".git").exists());
    fs::write(
        site.root.join("custom.toml"),
        "site_id = 'test'\nhostname = 'localhost'\n",
    )
    .unwrap();
    site.expect("sync", 3);
    site.expect("sync", 0);
    assert!(
        !fs::read_to_string(site._temp.path().join("docker.log"))
            .unwrap()
            .contains("compose")
    );
}

#[test]
fn runtime_volume_data_is_neither_hashed_nor_committed() {
    let site = Site::new();
    let config = fs::read_to_string(site.root.join("custom.toml")).unwrap();
    fs::write(
        site.root.join("custom.toml"),
        format!("volume_dir = './data'\n{config}"),
    )
    .unwrap();
    fs::create_dir(site.root.join("data")).unwrap();
    // A FIFO would fail fingerprinting (or hang a naive reader).
    let fifo =
        std::ffi::CString::new(site.root.join("data/runtime.fifo").to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    site.expect("commit", 3);
    assert!(!site.git(&["ls-files"]).contains("data/"));
    fs::write(site.root.join("data/database"), "runtime changes").unwrap();
    site.expect("sync", 0);
    assert!(site.git(&["status", "--porcelain"]).is_empty());
}

#[test]
fn enrollment_receipts_do_not_dirty_inputs_and_survive_update_baseline_reset() {
    let site = Site::new();
    fs::create_dir(site.root.join("pki")).unwrap();
    fs::write(site.root.join("pki/test.priv.pem"), "existing key").unwrap();
    site.expect("commit", 3);
    let local = fs::read(site.root.join("config.local.toml")).unwrap();
    let receipt = br#"{"enrolled_beam_networks":["broker.ccp-it.dktk.dkfz.de"]}"#;
    fs::write(site.root.join(".rusthead/enrollment.json"), receipt).unwrap();
    site.expect("sync", 0);
    assert_eq!(
        fs::read(site.root.join("config.local.toml")).unwrap(),
        local
    );
    let saved = fs::read(site.root.join(".rusthead/enrollment.json")).unwrap();
    fs::remove_file(site.root.join(".rusthead/state.json")).unwrap();
    site.expect("sync", 3);
    assert_eq!(
        fs::read(site.root.join(".rusthead/enrollment.json")).unwrap(),
        saved
    );
    assert!(!site.git(&["ls-files"]).contains("enrollment.json"));
}

#[test]
fn self_update_replaces_only_changed_binaries_and_resumes_once() {
    use std::{io::Write, os::unix::fs::MetadataExt};
    let mut site = Site::new();
    site.executable = site._temp.path().join("installed-rusthead");
    fs::copy(env!("CARGO_BIN_EXE_rusthead"), &site.executable).unwrap();
    let original_inode = fs::metadata(&site.executable).unwrap().ino();
    site.expect("commit", 3);
    site.expect("sync", 0);
    assert_eq!(
        original_inode,
        fs::metadata(&site.executable).unwrap().ino()
    );
    let log = fs::read_to_string(site._temp.path().join("docker.log")).unwrap();
    assert_eq!(
        log.lines()
            .filter(|line| line.starts_with("create "))
            .count(),
        1
    );
    // Simulate a new build without compiling a second Rust binary: ELF permits trailing data.
    let mut image = fs::OpenOptions::new()
        .append(true)
        .open(site._temp.path().join("image-rusthead"))
        .unwrap();
    image.write_all(b"new build").unwrap();
    drop(image);
    fs::write(site._temp.path().join("binary-id"), "two").unwrap();
    fs::write(site._temp.path().join("digest"), "two").unwrap();
    let out = site.expect("sync", 3);
    assert!(String::from_utf8_lossy(&out.stdout).contains("continuing with the updated version"));
    assert_ne!(
        original_inode,
        fs::metadata(&site.executable).unwrap().ino()
    );
    assert_eq!(
        fs::read(&site.executable).unwrap(),
        fs::read(site._temp.path().join("image-rusthead")).unwrap()
    );
    let log = fs::read_to_string(site._temp.path().join("docker.log")).unwrap();
    assert_eq!(
        log.lines()
            .filter(|line| line.starts_with("pull --platform"))
            .count(),
        3
    );
    assert_eq!(
        log.lines()
            .filter(|line| line.starts_with("create "))
            .count(),
        2
    );
    site.expect("sync", 0);
}

#[test]
fn self_update_failures_preserve_the_executable_and_clean_up_containers() {
    let mut site = Site::new();
    site.executable = site._temp.path().join("installed-rusthead");
    fs::copy(env!("CARGO_BIN_EXE_rusthead"), &site.executable).unwrap();
    let original = fs::read(&site.executable).unwrap();
    fs::write(site._temp.path().join("fail-binary-pull"), "").unwrap();
    site.expect("commit", 1);
    assert_eq!(original, fs::read(&site.executable).unwrap());
    fs::remove_file(site._temp.path().join("fail-binary-pull")).unwrap();
    fs::write(site._temp.path().join("fail-copy"), "").unwrap();
    site.expect("commit", 1);
    let log = fs::read_to_string(site._temp.path().join("docker.log")).unwrap();
    assert!(log.contains("rm --force extraction-container"));
    fs::remove_file(site._temp.path().join("fail-copy")).unwrap();
    fs::write(
        site._temp.path().join("image-rusthead"),
        "not an executable",
    )
    .unwrap();
    site.expect("commit", 1);
    assert_eq!(original, fs::read(&site.executable).unwrap());
    assert!(!site.root.join("services").exists());
}

#[test]
fn a_new_image_with_identical_binary_does_not_replace_and_dirty_sync_does_not_pull() {
    use std::os::unix::fs::MetadataExt;
    let mut site = Site::new();
    site.executable = site._temp.path().join("installed-rusthead");
    fs::copy(env!("CARGO_BIN_EXE_rusthead"), &site.executable).unwrap();
    let config = fs::read_to_string(site.root.join("custom.toml")).unwrap();
    fs::write(
        site.root.join("custom.toml"),
        format!("image = 'registry.example/rusthead:test'\n{config}"),
    )
    .unwrap();
    site.expect("commit", 3);
    let inode = fs::metadata(&site.executable).unwrap().ino();
    fs::write(
        site._temp.path().join("binary-id"),
        "different-image-same-binary",
    )
    .unwrap();
    site.expect("sync", 0);
    assert_eq!(inode, fs::metadata(&site.executable).unwrap().ino());
    let log = fs::read_to_string(site._temp.path().join("docker.log")).unwrap();
    assert!(log.contains("pull --platform linux/amd64 registry.example/rusthead:test"));
    assert!(log.contains("create --platform linux/amd64 sha256:different-image-same-binary"));
    site.append("custom.toml", "# unfinished edit\n");
    site.expect("sync", 1);
    assert_eq!(
        log,
        fs::read_to_string(site._temp.path().join("docker.log")).unwrap()
    );
}
