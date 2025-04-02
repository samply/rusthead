/// Helper for serde(default = "path_to_fn") as it does not work with constants
pub const fn enabled() -> bool {
    true
}

pub mod filters {
    use std::path::PathBuf;

    pub fn path(p: &PathBuf) -> askama::Result<String> {
        Ok(p.canonicalize()
            .map_err(|e| {
                askama::Error::custom(
                    anyhow::Error::from(e).context(format!("Failed to canonicalize {p:?}")),
                )
            })?
            .display()
            .to_string())
    }
}
