//! Durable receipts for completed enrollment, independent of update fingerprints.
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Enrollment {
    pub enrolled_beam_networks: BTreeSet<String>,
}

impl Enrollment {
    pub fn path(root: &Path) -> PathBuf {
        root.join(".rusthead/enrollment.json")
    }

    pub fn load(root: &Path) -> anyhow::Result<Self> {
        match fs::read(Self::path(root)) {
            Ok(bytes) => serde_json::from_slice(&bytes).context("Failed to parse enrollment.json"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).context("Failed to read enrollment.json"),
        }
    }

    pub fn save(&self, root: &Path) -> anyhow::Result<()> {
        let dir = crate::update_state::prepare_directory(root)?;
        let bytes = serde_json::to_vec_pretty(self)?;
        let path = Self::path(root);
        if fs::read(&path).ok().as_deref() == Some(&bytes) {
            return Ok(());
        }
        let tmp = dir.join("enrollment.json.tmp");
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o660)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&tmp)?;
        file.set_permissions(fs::Permissions::from_mode(0o660))?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(tmp, path).context("Failed to save enrollment.json")?;
        fs::File::open(dir)?.sync_all()?;
        Ok(())
    }
}
