use std::{
    collections::HashMap,
    fs,
    hash::{DefaultHasher, Hasher},
    process::Command,
};

use anyhow::Context;

use crate::config::Config;

fn git_command(conf: &Config) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(&conf.path);
    cmd
}

fn git_dirty(conf: &Config) -> anyhow::Result<bool> {
    Ok(!get_modified(conf)?.is_empty())
}

fn get_modified(conf: &Config) -> anyhow::Result<String> {
    let status = git_command(conf)
        .arg("status")
        .arg("--porcelain")
        .output()?;
    let files = String::from_utf8_lossy(&status.stdout);
    Ok(files.into_owned())
}

fn git_add_all(conf: &Config) -> anyhow::Result<()> {
    let status = git_command(conf).arg("add").arg(".").output()?;
    if !status.status.success() {
        anyhow::bail!(
            "Failed to add changes: {}",
            String::from_utf8_lossy(&status.stderr)
        );
    }
    Ok(())
}

fn stash_all(conf: &Config) -> anyhow::Result<()> {
    if is_initial_commit(conf)? {
        println!("No initial commit yet not stashing changes");
        return Ok(());
    }
    git_add_all(conf)?;
    let status = git_command(conf).arg("stash").output()?;
    if !status.status.success() {
        anyhow::bail!(
            "Failed to stash changes: {}",
            String::from_utf8_lossy(&status.stderr)
        );
    }
    Ok(())
}

pub fn stash_if_dirty(conf: &Config) -> anyhow::Result<()> {
    if git_dirty(conf)? {
        stash_all(conf)?;
    }
    Ok(())
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

/// Commit all changes to git. Return true if there were any changes to local or git tracked files.
pub fn commit_all(
    conf: &Config,
    before_hashes: &LocalDiffHashes,
    after_hashes: &LocalDiffHashes,
) -> anyhow::Result<bool> {
    let git_diff = get_modified(conf)?;
    let mut cmd = git_command(conf);
    cmd.arg("commit").arg("-m");
    let local_diff = compute_local_file_diff(before_hashes, after_hashes);
    let local_diff_str = local_diff
        .iter()
        .map(|(file, changed)| format!("{changed} {file}"))
        .collect::<Vec<_>>()
        .join("\n");
    match (git_diff.is_empty(), local_diff.is_empty()) {
        (true, true) => {
            cmd.arg("Nothing to update");
            cmd.arg("--allow-empty");
        }
        (true, false) => {
            cmd.arg(format!("Only local files changed\n\n{local_diff_str}"));
            cmd.arg("--allow-empty");
        }
        (false, true) => {
            git_add_all(conf)?;
            cmd.arg(format!("Git files changed\n\n{git_diff}"));
        }
        (false, false) => {
            git_add_all(conf)?;
            cmd.arg(format!(
                "Local files and git changed\n\n{git_diff}\nlocal:\n{local_diff_str}"
            ));
        }
    }
    let status = cmd.output()?;
    if !status.status.success() {
        anyhow::bail!(
            "Failed to commit changes: {}",
            String::from_utf8_lossy(&status.stdout)
        );
    }
    Ok(!(git_diff.is_empty() && local_diff.is_empty()))
}

pub type LocalDiffHashes = HashMap<String, u64>;

pub fn hash_untracked_files(conf: &Config) -> anyhow::Result<LocalDiffHashes> {
    let status = git_command(conf)
        .args(["ls-files", "--others", "--exclude-standard", "--ignored"])
        .output()?;
    if !status.status.success() {
        anyhow::bail!(
            "Failed to get status: {}",
            String::from_utf8_lossy(&status.stderr)
        );
    }
    let output = String::from_utf8_lossy(&status.stdout);
    let mut hash_map = LocalDiffHashes::default();
    for file_path in output.lines() {
        let mut hasher = DefaultHasher::new();
        let path = conf.path.join(file_path);
        let file = fs::read(&path)
            .with_context(|| format!("Failed to read file: `{}`", path.display()))?;
        hasher.write(&file);
        hash_map.insert(file_path.to_string(), hasher.finish());
    }
    Ok(hash_map)
}

fn is_initial_commit(conf: &Config) -> anyhow::Result<bool> {
    Ok(!git_command(conf)
        .arg("rev-parse")
        .arg("--verify")
        .arg("HEAD")
        .output()?
        .status
        .success())
}
