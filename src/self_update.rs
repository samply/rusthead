//! Pull a distribution image, compare executable content, and replace only when needed.
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::ExitCode,
};

const BINARY_PATH: &str = "/usr/local/bin/rusthead";
const HANDOFF: &str = "RUSTHEAD_SELF_UPDATE_HANDOFF";

#[derive(Deserialize, Serialize)]
struct InstalledImage {
    image_id: String,
    binary_hash: String,
}

pub struct Replacement {
    executable: PathBuf,
    hash: String,
}
impl Replacement {
    // The caller releases its update lock first. The child acquires that lock and
    // repeats preflight, while skipping this one already-completed image check.
    pub fn resume(self, config: &Path, mode: crate::update::Mode) -> anyhow::Result<ExitCode> {
        let mode = match mode {
            crate::update::Mode::Sync => "sync",
            crate::update::Mode::Commit => "commit",
        };
        let output = duct::cmd(
            &self.executable,
            [
                "--config".as_ref(),
                config.as_os_str(),
                "update".as_ref(),
                mode.as_ref(),
            ],
        )
        .env(HANDOFF, &self.hash)
        .unchecked()
        .run()
        .context("Failed to run the updated rusthead executable")?;
        let code = output
            .status
            .code()
            .context("Updated rusthead was killed by a signal")?;
        Ok(ExitCode::from(u8::try_from(code)?))
    }
}

fn hash_file(path: &Path) -> anyhow::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

struct Container(String);
impl Drop for Container {
    fn drop(&mut self) {
        if let Err(error) = duct::cmd!("docker", "rm", "--force", &self.0)
            .stdout_capture()
            .stderr_capture()
            .run()
        {
            eprintln!("Failed to remove self-update container {}: {error}", self.0);
        }
    }
}

pub fn check(conf: &crate::Config) -> anyhow::Result<Option<Replacement>> {
    ensure!(
        cfg!(all(target_os = "linux", target_arch = "x86_64")),
        "Binary self-update supports Linux x86_64 only"
    );
    let executable = std::env::current_exe().context("Cannot locate the running executable")?;
    let current_hash = hash_file(&executable)?;
    if std::env::var(HANDOFF).ok().as_deref() == Some(&current_hash) {
        return Ok(None);
    }
    ensure!(
        !conf.image.is_empty() && !conf.image.starts_with('-'),
        "Invalid rusthead distribution image"
    );
    println!("Checking rusthead image {}", conf.image);
    duct::cmd!("docker", "pull", "--platform", "linux/amd64", &conf.image)
        .run()
        .context("Failed to pull rusthead distribution image")?;
    let inspection = duct::cmd!(
        "docker",
        "image",
        "inspect",
        "--format",
        "{{.Os}}/{{.Architecture}} {{.Id}}",
        &conf.image
    )
    .read()
    .context("Failed to inspect rusthead distribution image")?;
    let (platform, image_id) = inspection
        .split_once(' ')
        .context("Invalid Docker image inspection result")?;
    ensure!(
        platform == "linux/amd64" && image_id.starts_with("sha256:"),
        "Rusthead distribution image must target linux/amd64"
    );
    let metadata_path = conf.path.join(".rusthead/binary.json");
    let cached = fs::read(&metadata_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<InstalledImage>(&bytes).ok());
    if cached
        .is_some_and(|cached| cached.image_id == image_id && cached.binary_hash == current_hash)
    {
        return Ok(None);
    }
    let temp = tempfile::tempdir().context("Cannot prepare binary extraction directory")?;
    // Create by immutable ID so a concurrent retag cannot change the extracted artifact.
    let id = duct::cmd!("docker", "create", "--platform", "linux/amd64", image_id)
        .read()
        .context("Failed to create rusthead extraction container")?;
    ensure!(
        !id.is_empty() && !id.starts_with('-') && !id.contains(char::is_whitespace),
        "Invalid extraction container ID"
    );
    let container = Container(id);
    let candidate = temp.path().join("rusthead");
    duct::cmd!(
        "docker",
        "cp",
        format!("{}:{BINARY_PATH}", container.0),
        &candidate
    )
    .run()
    .context("Failed to extract rusthead executable")?;
    drop(container); // The distribution container is never started.
    ensure!(
        fs::symlink_metadata(&candidate)?.is_file(),
        "Extracted rusthead must be a regular file"
    );
    let new_hash = hash_file(&candidate)?;
    if new_hash != current_hash {
        let mut header = [0u8; 20];
        fs::File::open(&candidate)?
            .read_exact(&mut header)
            .context("Extracted executable is truncated")?;
        ensure!(
            &header[..4] == b"\x7fELF"
                && header[4] == 2
                && header[5] == 1
                && header[18..20] == [62, 0],
            "Extracted executable is not an x86_64 ELF binary"
        );
        fs::set_permissions(&candidate, fs::Permissions::from_mode(0o755))?;
        duct::cmd!(&candidate, "--version")
            .stdout_capture()
            .stderr_capture()
            .run()
            .context("Extracted rusthead executable could not run")?;
        self_replace::self_replace(&candidate).with_context(|| format!("Failed to replace {}; the executable directory must be writable by the update user", executable.display()))?;
        println!("Installed a new rusthead binary; continuing with the updated version.");
    }
    let metadata = InstalledImage {
        image_id: image_id.to_owned(),
        binary_hash: new_hash.clone(),
    };
    let tmp = metadata_path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec(&metadata)?)?;
    fs::rename(tmp, metadata_path)?;
    Ok((new_hash != current_hash).then_some(Replacement {
        executable,
        hash: new_hash,
    }))
}
