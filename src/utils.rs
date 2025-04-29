/// Helper for serde(default = "path_to_fn") as it does not work with constants
pub const fn enabled() -> bool {
    true
}

pub mod filters {
    use std::path::PathBuf;

    use askama::Values;

    pub fn path(p: &PathBuf, _: &dyn Values) -> askama::Result<String> {
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

pub mod host {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use serde::Deserialize;
    use url::Host;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Host, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        pub enum HostHelper {
            Ipv4(Ipv4Addr),
            Ipv6(Ipv6Addr),
            Domain(String),
        }
        match HostHelper::deserialize(deserializer)? {
            HostHelper::Ipv4(ipv4_addr) => Ok(Host::Ipv4(ipv4_addr)),
            HostHelper::Ipv6(ipv6_addr) => Ok(Host::Ipv6(ipv6_addr)),
            HostHelper::Domain(d) => Ok(Host::Domain(d)),
        }
    }
}
