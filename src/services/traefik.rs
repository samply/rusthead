use std::path::PathBuf;

use rinja::Template;

use super::Service;

#[derive(Debug, Template)]
#[template(path = "traefik.yml")]
pub struct Traefik {
    tls_dir: PathBuf,
}

impl Service for Traefik {
    type Dependencies<'s> = ();

    fn from_config(_conf: &crate::Config, _deps: super::Deps<'_, Self>) -> Self {
        Self {
            tls_dir: "/etc/bridgehead/traefik-tls".into(),
        }
    }

    fn service_name() -> String {
        "traefik".into()
    }
}
