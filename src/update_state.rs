//! Private, persistent fingerprints; never a Git cleanliness proxy.
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    os::{
        fd::AsRawFd,
        unix::fs::{OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
};

pub type Fingerprints = BTreeMap<String, String>;
pub const LOCAL_INPUTS: &[&str] = &[
    "config.local.toml",
    "docker-compose.override.yml",
    "trusted-ca-certs",
];
pub const OUTPUTS: &[&str] = &["services", "docker-image.lock.yml", ".env"];

#[derive(Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub local_inputs: Fingerprints,
    pub outputs: Fingerprints,
}

pub struct UpdateLock {
    _file: File,
}
impl UpdateLock {
    pub fn acquire(root: &Path) -> anyhow::Result<Self> {
        let dir = prepare_directory(root)?;
        let open_lock = |create| {
            OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(create)
                .mode(0o660)
                .custom_flags(libc::O_NOFOLLOW)
                .open(dir.join("update.lock"))
        };
        let file = match open_lock(true) {
            Ok(file) => {
                file.set_permissions(fs::Permissions::from_mode(0o660))?;
                file
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => open_lock(false)?,
            Err(e) => return Err(e.into()),
        };
        ensure!(
            unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
            "Another update is running (or the update lock could not be acquired)"
        );
        Ok(Self { _file: file })
    }
}

/// Shared private metadata directory; enrollment receipts outlive update baselines.
pub(crate) fn prepare_directory(root: &Path) -> anyhow::Result<PathBuf> {
    let dir = root.join(".rusthead");
    match fs::create_dir(&dir) {
        Ok(()) => fs::set_permissions(&dir, fs::Permissions::from_mode(0o2770))?,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            ensure!(
                fs::symlink_metadata(&dir)?.is_dir(),
                ".rusthead must be a directory, not a symlink"
            );
            // The other installation user may own this shared directory.
        }
        Err(e) => return Err(e.into()),
    }
    Ok(dir)
}

impl State {
    pub fn runtime_changed(&self, other: &Self) -> bool {
        // config.local affects runtime through the generated .env and service files.
        self.outputs != other.outputs
            || self
                .local_inputs
                .iter()
                .filter(|(key, _)| key.as_str() != "config.local.toml")
                .ne(other
                    .local_inputs
                    .iter()
                    .filter(|(key, _)| key.as_str() != "config.local.toml"))
    }

    pub fn load(root: &Path) -> anyhow::Result<Option<Self>> {
        match fs::read(root.join(".rusthead/state.json")) {
            Ok(bytes) => Ok(Some(
                serde_json::from_slice(&bytes).context("Cannot read update baseline")?,
            )),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    pub fn save(&self, root: &Path) -> anyhow::Result<()> {
        let tmp = root.join(".rusthead/state.json.tmp");
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o660)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&tmp)?;
        use std::io::Write;
        file.write_all(&serde_json::to_vec_pretty(self)?)?;
        file.sync_all()?;
        fs::rename(tmp, root.join(".rusthead/state.json"))?;
        Ok(())
    }
}

pub fn fingerprint(
    root: &Path,
    paths: impl IntoIterator<Item = PathBuf>,
) -> anyhow::Result<Fingerprints> {
    let mut result = Fingerprints::new();
    for path in paths {
        let full = root.join(&path);
        let metadata = match fs::metadata(&full) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e).with_context(|| format!("Cannot inspect {}", full.display())),
        };
        if metadata.is_dir() {
            // Generated services and trust certificates are flat directories.
            for entry in fs::read_dir(&full)? {
                fingerprint_file(root, &path.join(entry?.file_name()), &mut result)?;
            }
        } else {
            fingerprint_file(root, &path, &mut result)?;
        }
    }
    Ok(result)
}

fn fingerprint_file(root: &Path, path: &Path, result: &mut Fingerprints) -> anyhow::Result<()> {
    let full = root.join(path);
    ensure!(
        fs::metadata(&full)?.is_file(),
        "Expected a file while fingerprinting {}",
        full.display()
    );
    let key = path
        .to_str()
        .context("Update fingerprint paths must be UTF-8")?
        .to_owned();
    if fs::symlink_metadata(&full)?.file_type().is_symlink() {
        result.insert(
            format!("{key}/@symlink"),
            fs::read_link(&full)?.to_string_lossy().into_owned(),
        );
    }
    let contents =
        fs::read(&full).with_context(|| format!("Cannot fingerprint {}", full.display()))?;
    result.insert(key, format!("{:x}", Sha256::digest(contents)));
    Ok(())
}

pub fn local_paths() -> Vec<PathBuf> {
    LOCAL_INPUTS.iter().map(PathBuf::from).collect()
}

pub fn ensure_ignore(conf: &crate::Config) -> anyhow::Result<()> {
    let root = &conf.path;
    let path = root.join(".gitignore");
    let mut contents = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    // Keep custom entries; put mandatory rules last so earlier negations cannot expose secrets.
    let mut rules = format!(
        "{}\n/.rusthead/\n",
        include_str!("../static/.gitignore").trim_end()
    );
    if let Some(volume) = volume_path(conf)?
        && let Ok(relative) = volume.strip_prefix(root)
    {
        let relative = relative
            .to_str()
            .context("Volume directory must be UTF-8")?;
        ensure!(
            !relative.chars().any(char::is_control),
            "Volume directory cannot contain control characters"
        );
        let escaped: String = relative
            .chars()
            .flat_map(|ch| {
                if matches!(ch, '\\' | '*' | '?' | '[' | ']' | ' ' | '!' | '#') {
                    vec!['\\', ch]
                } else {
                    vec![ch]
                }
            })
            .collect();
        rules.push_str(&format!("/{escaped}/\n"));
    }
    if !contents.ends_with(&rules) {
        if !contents.is_empty() && !contents.ends_with('\n') {
            contents.push('\n');
        }
        contents.push_str(&rules);
        fs::write(path, contents)?;
    }
    Ok(())
}

// A failed image pull can leave valid generated files on disk. Permit an explicit
// commit retry only while those files still match what this process generated.
pub fn record_pending(root: &Path) -> anyhow::Result<Fingerprints> {
    let outputs = fingerprint(root, OUTPUTS.iter().map(PathBuf::from))?;
    let path = root.join(".rusthead/pending.json");
    use std::io::Write;
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o660)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    file.write_all(&serde_json::to_vec(&outputs)?)?;
    Ok(outputs)
}
pub fn pending_matches(root: &Path, outputs: &Fingerprints) -> anyhow::Result<bool> {
    match fs::read(root.join(".rusthead/pending.json")) {
        Ok(bytes) => Ok(serde_json::from_slice::<Fingerprints>(&bytes)? == *outputs),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// Resolve the configured data directory without requiring it to exist yet.
pub fn volume_path(conf: &crate::Config) -> anyhow::Result<Option<PathBuf>> {
    let Some(volume) = &conf.volume_dir else {
        return Ok(None);
    };
    let mut path = PathBuf::new();
    for component in conf.path.join(volume).components() {
        match component {
            std::path::Component::ParentDir => {
                path.pop();
            }
            std::path::Component::CurDir => {}
            component => path.push(component.as_os_str()),
        }
    }
    ensure!(
        !conf.path.starts_with(&path),
        "volume_dir must be outside the installation or a dedicated subdirectory, not the installation directory or an ancestor"
    );
    Ok(Some(path))
}
