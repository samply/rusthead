/// Helper for serde(default = "path_to_fn") as it does not work with constants
pub const fn enabled() -> bool {
    true
}

pub fn capitalize_first_letter(s: &str) -> String {
    let mut chars = s.chars();
    chars
        .next()
        .map(char::to_uppercase)
        .into_iter()
        .flatten()
        .chain(chars)
        .collect()
}

pub fn secret_from_rng<const N: usize>(rng: &mut impl rand::Rng) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                            abcdefghijklmnopqrstuvwxyz\
                            0123456789)(*&^%#@!~";
    (0..N)
        .map(|_| CHARSET[rng.random_range(0..CHARSET.len())] as char)
        .collect()
}

pub mod filters {
    use std::path::PathBuf;

    use askama::Values;

    #[askama::filter_fn]
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
