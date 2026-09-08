//! Git history and transport. Generation and runtime change detection live elsewhere.
use anyhow::{Context, ensure};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Output,
};

pub struct Repository {
    pub root: PathBuf,
}

impl Repository {
    pub fn open(root: &Path) -> anyhow::Result<Option<Self>> {
        let repo = Self {
            root: root.to_owned(),
        };
        // Do not silently join an enclosing repository, including through worktrees.
        let out = repo.output(&["rev-parse", "--show-toplevel"])?;
        if !out.status.success() {
            ensure!(
                !root.join(".git").exists(),
                "Cannot read installation Git repository: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            // A failed discovery (e.g. dubious ownership) must not trigger reinitialization.
            ensure!(
                String::from_utf8_lossy(&out.stderr).contains("not a git repository"),
                "Git discovery failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            return Ok(None);
        }
        let top = PathBuf::from(String::from_utf8(out.stdout)?.trim_end_matches('\n'));
        ensure!(
            top.canonicalize()? == root.canonicalize()?,
            "The installation directory must be the Git repository root"
        );
        Ok(Some(repo))
    }

    pub fn initialize(root: &Path) -> anyhow::Result<Self> {
        let repo = match Self::open(root)? {
            Some(repo) => repo,
            None => {
                let repo = Self {
                    root: root.to_owned(),
                };
                repo.run(&["init", "-b", "main", "--shared=group"])?;
                repo
            }
        };
        for (key, fallback) in [
            ("user.name", "Bridgehead"),
            ("user.email", "bridgehead@samply.de"),
        ] {
            if !repo.output(&["config", "--get", key])?.status.success() {
                repo.run(&["config", "--local", key, fallback])?;
            }
        }
        Ok(repo)
    }

    pub fn output(&self, args: &[&str]) -> anyhow::Result<Output> {
        duct::cmd("git", args)
            .env("LC_ALL", "C")
            .dir(&self.root)
            .stdout_capture()
            .stderr_capture()
            .unchecked()
            .run()
            .context("Failed to execute git")
    }
    pub fn run(&self, args: &[&str]) -> anyhow::Result<Vec<u8>> {
        let out = self.output(args)?;
        ensure!(
            out.status.success(),
            "git {} failed: {}{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        Ok(out.stdout)
    }
    pub fn has_head(&self) -> anyhow::Result<bool> {
        Ok(self
            .output(&["rev-parse", "--verify", "HEAD"])?
            .status
            .success())
    }
    pub fn head(&self) -> anyhow::Result<Vec<u8>> {
        if self.has_head()? {
            self.run(&["rev-parse", "HEAD"])
        } else {
            Ok(Vec::new())
        }
    }
    pub fn ensure_idle(&self) -> anyhow::Result<()> {
        for marker in [
            "MERGE_HEAD",
            "CHERRY_PICK_HEAD",
            "REVERT_HEAD",
            "rebase-merge",
            "rebase-apply",
            "sequencer",
        ] {
            let path = self.run(&["rev-parse", "--git-path", marker])?;
            let path = String::from_utf8(path)?;
            ensure!(
                !self.root.join(path.trim_end()).exists(),
                "Finish or abort the current Git operation before updating ({marker})"
            );
        }
        ensure!(
            self.run(&["ls-files", "-u"])?.is_empty(),
            "Resolve Git conflicts before updating"
        );
        ensure!(
            self.run(&["ls-files", "--", ".rusthead"])?.is_empty(),
            "Internal .rusthead state must not be tracked by Git"
        );
        ensure!(
            self.run(&[
                "ls-files",
                "--",
                ".env",
                "config.local.toml",
                "pki",
                "trusted-ca-certs",
                "traefik-tls"
            ])?
            .is_empty(),
            "Local credentials and certificates must be untracked before updating"
        );
        Ok(())
    }
    pub fn dirty(&self) -> anyhow::Result<bool> {
        Ok(!self
            .run(&[
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--",
                ".",
                ":(exclude).rusthead",
            ])?
            .is_empty())
    }
    pub fn generated_dirty(&self) -> anyhow::Result<bool> {
        Ok(!self
            .run(&[
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--",
                "services",
                "docker-image.lock.yml",
            ])?
            .is_empty())
    }
    pub fn input_paths(&self) -> anyhow::Result<Vec<PathBuf>> {
        use std::os::unix::ffi::OsStringExt;
        let files = self.run(&[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])?;
        Ok(files
            .split(|b| *b == 0)
            .filter(|p| !p.is_empty())
            .map(|p| PathBuf::from(OsString::from_vec(p.to_vec())))
            .filter(|p| {
                !p.starts_with("services")
                    && !p.starts_with(".rusthead")
                    && p != Path::new("docker-image.lock.yml")
                    && p != Path::new(".env")
            })
            .collect())
    }
    pub fn upstream(&self) -> anyhow::Result<(String, String)> {
        let branch = self
            .run(&["symbolic-ref", "--quiet", "--short", "HEAD"])
            .context("git_sync requires a branch with an upstream")?;
        let branch = String::from_utf8(branch)?.trim().to_owned();
        let remote = String::from_utf8(
            self.run(&["config", "--get", &format!("branch.{branch}.remote")])
                .context("git_sync requires a configured upstream")?,
        )?
        .trim()
        .to_owned();
        let target = String::from_utf8(
            self.run(&["config", "--get", &format!("branch.{branch}.merge")])
                .context("git_sync requires a configured upstream")?,
        )?
        .trim()
        .to_owned();
        ensure!(
            !remote.is_empty() && !remote.starts_with('-') && target.starts_with("refs/heads/"),
            "Unsupported Git upstream configuration"
        );
        Ok((remote, target))
    }
    pub fn sync(&self, upstream: &(String, String)) -> anyhow::Result<()> {
        self.run(&["fetch", &upstream.0, &upstream.1])?;
        if !self.has_head()? {
            anyhow::bail!(
                "Create the initial local commit with update commit before pulling an upstream"
            );
        }
        if self
            .output(&["merge-base", "--is-ancestor", "FETCH_HEAD", "HEAD"])?
            .status
            .success()
        {
            return Ok(()); // equal or locally ahead: publish after generation
        }
        ensure!(
            self.output(&["merge-base", "--is-ancestor", "HEAD", "FETCH_HEAD"])?
                .status
                .success(),
            "Local and upstream histories diverged; reconcile them manually and retry"
        );
        self.run(&["merge", "--ff-only", "FETCH_HEAD"])?;
        Ok(())
    }
    pub fn commit(&self) -> anyhow::Result<()> {
        self.run(&["add", "-A", "--", "."])?;
        let diff = self.output(&["diff", "--cached", "--quiet"])?;
        match diff.status.code() {
            Some(0) if self.has_head()? => return Ok(()),
            Some(0 | 1) => {}
            _ => anyhow::bail!(
                "Failed to inspect staged changes: {}",
                String::from_utf8_lossy(&diff.stderr)
            ),
        }
        self.run(&[
            "commit",
            "--allow-empty",
            "-m",
            "Update bridgehead configuration",
        ])?;
        Ok(())
    }
    pub fn push(&self, upstream: &(String, String)) -> anyhow::Result<()> {
        self.run(&["push", &upstream.0, &format!("HEAD:{}", upstream.1)])?;
        Ok(())
    }
}
