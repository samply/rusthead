//! Private, persistent fingerprints; never a Git cleanliness proxy.
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
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
    "pki",
    "trusted-ca-certs",
    "traefik-tls",
];
pub const OUTPUTS: &[&str] = &["services", "docker-image.lock.yml", ".env"];

#[derive(Default, Serialize, Deserialize)]
pub struct State {
    pub inputs: Fingerprints,
    pub local_inputs: Fingerprints,
    pub outputs: Fingerprints,
    pub runtime: Fingerprints,
}

pub struct UpdateLock {
    _file: File,
}
impl UpdateLock {
    pub fn acquire(root: &Path) -> anyhow::Result<Self> {
        let dir = root.join(".rusthead");
        if dir.exists() {
            ensure!(
                !fs::symlink_metadata(&dir)?.file_type().is_symlink(),
                ".rusthead must not be a symlink"
            );
        }
        fs::create_dir_all(&dir)?;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o2770))?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o660)
            .custom_flags(libc::O_NOFOLLOW)
            .open(dir.join("update.lock"))?;
        ensure!(
            unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
            "Another update is running (or the update lock could not be acquired)"
        );
        Ok(Self { _file: file })
    }
}

impl State {
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
        visit(root, &path, &mut result, &mut BTreeSet::new())?;
    }
    Ok(result)
}
fn visit(
    root: &Path,
    path: &Path,
    result: &mut Fingerprints,
    ancestors: &mut BTreeSet<PathBuf>,
) -> anyhow::Result<()> {
    let full = root.join(path);
    let meta = match fs::symlink_metadata(&full) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e).with_context(|| format!("Cannot inspect {}", full.display())),
    };
    let key = path
        .to_str()
        .context("Update fingerprint paths must be UTF-8")?
        .to_owned();
    if meta.file_type().is_symlink() {
        let target = fs::read_link(&full)?;
        result.insert(
            format!("{key}/@symlink"),
            target.to_string_lossy().into_owned(),
        );
    }
    let meta = fs::metadata(&full).with_context(|| format!("Cannot follow {}", full.display()))?;
    if meta.is_dir() {
        let canonical = full.canonicalize()?;
        ensure!(
            ancestors.insert(canonical.clone()),
            "Symlink cycle at {}",
            full.display()
        );
        for entry in fs::read_dir(full)? {
            visit(root, &path.join(entry?.file_name()), result, ancestors)?;
        }
        ancestors.remove(&canonical);
    } else {
        ensure!(
            meta.is_file(),
            "Cannot fingerprint special file {}",
            full.display()
        );
        result.insert(key, format!("{:x}", Sha256::digest(fs::read(&full)?)));
    }
    Ok(())
}

pub fn local_paths(conf: &crate::Config) -> Vec<PathBuf> {
    let mut paths: Vec<_> = LOCAL_INPUTS.iter().map(PathBuf::from).collect();
    if let Some(tls) = conf.traefik.as_ref().and_then(|t| t.tls.as_ref()) {
        paths.extend([tls.cert_file.clone(), tls.key_file.clone()]);
    }
    paths
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
