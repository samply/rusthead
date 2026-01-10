use std::{
    collections::HashMap,
    fs,
    hash::{DefaultHasher, Hasher},
    process::Command,
};

use anyhow::Context;

use crate::config::Config;

fn is_git_repo(conf: &Config) -> bool {
    fs::metadata(conf.path.join(".git")).map_or(false, |meta| meta.is_dir())
}

type LocalDiffHashes = HashMap<String, u64>;

pub struct DiffTracker<'a> {
    conf: &'a Config,
    before_hashes: LocalDiffHashes,
    stashed_changes: Option<String>,
}

pub enum DiffTrackerResult<'a> {
    Success(DiffTracker<'a>),
    NeedsConfigReload,
    NotAGitRepo,
}

impl<'a> DiffTracker<'a> {
    pub fn start(conf: &'a Config) -> anyhow::Result<DiffTrackerResult<'a>> {
        if !is_git_repo(conf) {
            println!("Directory is not a git repository yet skipping diff tracking");
            return Ok(DiffTrackerResult::NotAGitRepo);
        }
        let tmp_self = Self {
            conf,
            before_hashes: LocalDiffHashes::default(),
            stashed_changes: None,
        };
        let git_diff = tmp_self.get_modified()?;
        let stashed_changes = if !git_diff.is_empty() {
            if tmp_self.is_initial_commit()? {
                println!("No initial commit yet not stashing changes");
                None
            } else {
                tmp_self.stash_all()?;
                Some(git_diff)
            }
        } else {
            None
        };
        if conf.git_sync.unwrap_or_else(|| tmp_self.has_remote()) {
            let repo_hash_before = tmp_self.head_hash()?.stdout;
            println!("Pulling changes from remote");
            tmp_self.pull()?;
            let repo_hash_after = tmp_self.head_hash()?.stdout;
            if repo_hash_before != repo_hash_after {
                return Ok(DiffTrackerResult::NeedsConfigReload);
            }
        }
        Ok(DiffTrackerResult::Success(Self {
            stashed_changes,
            before_hashes: tmp_self
                .hash_untracked_files()
                .context("Failed to start tracking local files")?,
            ..tmp_self
        }))
    }

    fn git_command(&self) -> Command {
        let mut cmd = Command::new("git");
        cmd.current_dir(&self.conf.path);
        cmd
    }

    fn get_modified(&self) -> anyhow::Result<String> {
        let status = self
            .git_command()
            .arg("status")
            .arg("--porcelain")
            .output()?;
        if !status.status.success() {
            anyhow::bail!(
                "Failed to get status: {}",
                String::from_utf8_lossy(&status.stderr)
            );
        }
        let files = String::from_utf8_lossy(&status.stdout);
        Ok(files.into_owned())
    }

    fn hash_untracked_files(&self) -> anyhow::Result<LocalDiffHashes> {
        let status = self
            .git_command()
            .args(["ls-files", "--others", "--exclude-standard", "--ignored"])
            .output()?;
        if !status.status.success() {
            anyhow::bail!(
                "Failed to get untracked files: {}",
                String::from_utf8_lossy(&status.stderr)
            );
        }
        let output = String::from_utf8_lossy(&status.stdout);
        let mut hash_map = LocalDiffHashes::default();
        for file_path in output.lines() {
            let mut hasher = DefaultHasher::new();
            let path = self.conf.path.join(file_path);
            let file = fs::read(&path)
                .with_context(|| format!("Failed to read file: `{}`", path.display()))?;
            hasher.write(&file);
            hash_map.insert(file_path.to_string(), hasher.finish());
        }
        Ok(hash_map)
    }

    fn is_initial_commit(&self) -> anyhow::Result<bool> {
        Ok(!self.head_hash()?.status.success())
    }

    fn head_hash(&self) -> anyhow::Result<std::process::Output> {
        Ok(self
            .git_command()
            .arg("rev-parse")
            .arg("--verify")
            .arg("HEAD")
            .output()?)
    }

    fn stash_all(&self) -> anyhow::Result<()> {
        println!("Stashing untracked changes:\n{}", self.get_modified()?);
        let status = self
            .git_command()
            .args(["stash", "push", "-m", "auto-stash", "--include-untracked"])
            .output()?;
        if !status.status.success() {
            anyhow::bail!(
                "Failed to stash changes: {}",
                String::from_utf8_lossy(&status.stderr)
            );
        }
        Ok(())
    }

    fn git_add_all(&self) -> anyhow::Result<()> {
        let status = self.git_command().arg("add").arg(".").output()?;
        if !status.status.success() {
            anyhow::bail!(
                "Failed to add changes: {}",
                String::from_utf8_lossy(&status.stderr)
            );
        }
        Ok(())
    }

    /// Commit all changes to git. Return true if there were any changes to local or git tracked files.
    pub fn commit(self) -> anyhow::Result<bool> {
        let git_diff = self.get_modified()?;
        let after_hashes = self.hash_untracked_files()?;
        let local_diff = compute_local_file_diff(&self.before_hashes, &after_hashes);
        let local_diff_str = local_diff
            .iter()
            .map(|(file, changed)| format!("{changed} {file}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut cmd = self.git_command();
        cmd.arg("commit").arg("-m");
        match (git_diff.is_empty(), local_diff.is_empty()) {
            (true, true) => {
                cmd.arg("Nothing changed");
                cmd.arg("--allow-empty");
            }
            (true, false) => {
                cmd.arg(format!(
                    "Only local files changed\n\nlocal:\n{local_diff_str}"
                ));
                cmd.arg("--allow-empty");
            }
            (false, true) => {
                self.git_add_all()?;
                cmd.arg(format!("Git files changed\n\ngit:\n{git_diff}"));
            }
            (false, false) => {
                self.git_add_all()?;
                cmd.arg(format!(
                    "Local files and git changed\n\ngit:\n{git_diff}\nlocal:\n{local_diff_str}"
                ));
            }
        }
        if let Some(ref stashed_changes) = self.stashed_changes {
            cmd.arg("-m")
                .arg(format!("stashed changes:\n{stashed_changes}"));
        }
        let status = cmd.output()?;
        if !status.status.success() {
            anyhow::bail!(
                "Failed to commit changes: {}",
                String::from_utf8_lossy(&status.stdout)
            );
        }
        if self.conf.git_sync.unwrap_or_else(|| self.has_remote()) {
            println!("Pushing changes to remote");
            self.push()?;
        }
        Ok(!(git_diff.is_empty() && local_diff.is_empty()))
    }

    fn has_remote(&self) -> bool {
        self.git_command()
            .arg("remote")
            .output()
            .is_ok_and(|output| output.status.success() && !output.stdout.is_empty())
    }

    fn pull(&self) -> anyhow::Result<()> {
        let output = self.git_command().arg("pull").arg("--rebase").output()?;
        if !output.status.success() {
            anyhow::bail!(
                "Failed to pull changes: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }

    fn push(&self) -> anyhow::Result<()> {
        let output = self.git_command().arg("push").output()?;
        if !output.status.success() {
            anyhow::bail!(
                "Failed to push changes: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }
}

fn compute_local_file_diff<'a>(
    before_hashes: &'a LocalDiffHashes,
    after_hashes: &'a LocalDiffHashes,
) -> HashMap<&'a str, char> {
    let mut diff = HashMap::new();

    for (file, &before_hash) in before_hashes {
        if let Some(&after_hash) = after_hashes.get(file) {
            if before_hash == after_hash {
                continue;
            } else {
                diff.insert(file.as_str(), 'M');
            }
        } else {
            diff.insert(file.as_str(), 'D');
        }
    }

    for file in after_hashes.keys() {
        if !before_hashes.contains_key(file) {
            diff.insert(file.as_str(), 'A');
        }
    }

    diff
}
