use std::{cell::RefCell, fs, path::PathBuf};

use askama::Template;
use bcrypt::DEFAULT_COST;
use rcgen::CertifiedKey;
use serde::{Deserialize, Serialize};

use crate::{config::LocalConf, utils::filters};

use super::Service;

#[derive(Debug, Deserialize)]
pub struct TraefikConfig {
    tls: Option<TlsConfig>,
}

#[derive(Debug, Deserialize, Clone)]
struct TlsConfig {
    cert_file: PathBuf,
    key_file: PathBuf,
}

#[derive(Debug, Template)]
#[template(path = "traefik.yml")]
pub struct Traefik {
    tls: TlsConfig,
    local_conf: &'static RefCell<LocalConf>,
}

impl Traefik {
    pub fn add_basic_auth_user(&mut self, middleware_name: String) {
        let mut local_conf = self.local_conf.borrow_mut();
        let pw = crate::utils::secret_from_rng::<10>(&mut rand::rng());
        local_conf
            .basic_auth_users
            .get_or_insert_default()
            .entry(middleware_name)
            .or_insert_with(|| {
                if cfg!(test) {
                    BasicAuthUser {
                        hash: "<hash>".into(),
                        pw: Some("test".into()),
                    }
                } else {
                    let hash = bcrypt::hash(&pw, DEFAULT_COST).unwrap();
                    BasicAuthUser { hash, pw: Some(pw) }
                }
            });
    }
}

impl Service for Traefik {
    type Dependencies = ();
    type ServiceConfig = &'static crate::Config;

    fn from_config(conf: Self::ServiceConfig, _deps: super::Deps<Self>) -> Self {
        let tls = if let Some(tls) = conf.traefik.as_ref().and_then(|t| t.tls.as_ref()) {
            // We don't check if the certs exist as they might not be mounted into the container
            tls.clone()
        } else {
            let tls_dir = conf.path.join("traefik-tls");
            fs::create_dir_all(&tls_dir).unwrap();
            let tls = TlsConfig {
                cert_file: tls_dir.join("fullchain.pem"),
                key_file: tls_dir.join("privkey.pem"),
            };
            match (
                fs::exists(&tls.cert_file).unwrap(),
                fs::exists(&tls.key_file).unwrap(),
            ) {
                (false, false) => {
                    eprintln!(
                        "No ssl certs found for traefik {tls_dir:?}. Generating self-signed certificate"
                    );
                    let CertifiedKey { cert, signing_key } =
                        rcgen::generate_simple_self_signed(vec![conf.hostname.to_string()])
                            .unwrap();
                    fs::write(&tls.cert_file, cert.pem()).unwrap();
                    fs::write(&tls.key_file, signing_key.serialize_pem()).unwrap();
                }
                (true, false) => {
                    panic!("fullchain.pem exists but privkey.pem does not");
                }
                (false, true) => {
                    panic!("privkey.pem exists but fullchain.pem does not");
                }
                (true, true) => {}
            };
            tls
        };
        Self {
            tls,
            local_conf: &conf.local_conf,
        }
    }

    fn service_name() -> String {
        "traefik".into()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BasicAuthUser {
    pub hash: String,
    pw: Option<String>,
}
