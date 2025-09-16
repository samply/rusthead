use std::{cell::RefCell, fs, path::PathBuf};

use askama::Template;
use bcrypt::DEFAULT_COST;
use rcgen::CertifiedKey;
use serde::{Deserialize, Serialize};

use crate::{config::LocalConf, utils::filters};

use super::Service;

#[derive(Debug, Template)]
#[template(path = "traefik.yml")]
pub struct Traefik {
    tls_dir: PathBuf,
    local_conf: &'static RefCell<LocalConf>,
}

impl Traefik {
    // TODO: persist to some local.config.toml or smth maybe with toml_edit
    pub fn add_basic_auth_user(&mut self, middleware_name: String) {
        let mut local_conf = self.local_conf.borrow_mut();
        let pw = crate::utils::secret_from_rng::<10>(&mut rand::rng());
        local_conf
            .basic_auth_users
            .get_or_insert_default()
            .entry(middleware_name)
            .or_insert_with(|| {
                let hash = bcrypt::hash(&pw, DEFAULT_COST).unwrap();
                BasicAuthUser { hash, pw: Some(pw) }
            });
    }
}

impl Service for Traefik {
    type Dependencies = ();
    type ServiceConfig = &'static crate::Config;

    fn from_config(conf: Self::ServiceConfig, _deps: super::Deps<Self>) -> Self {
        let tls_dir = conf.path.join("traefik-tls");
        fs::create_dir_all(&tls_dir).unwrap();
        let full_chain = tls_dir.join("fullchain.pem");
        let priv_key = tls_dir.join("privkey.pem");
        match (
            fs::exists(&full_chain).unwrap(),
            fs::exists(&priv_key).unwrap(),
        ) {
            (false, false) => {
                eprintln!(
                    "No ssl certs found for traefik in {tls_dir:?}. Generating self-signed certificate"
                );
                let CertifiedKey { cert, signing_key } =
                    rcgen::generate_simple_self_signed(vec![conf.hostname.to_string()]).unwrap();
                fs::write(full_chain, cert.pem()).unwrap();
                fs::write(priv_key, signing_key.serialize_pem()).unwrap();
            }
            (true, false) => {
                panic!("fullchain.pem exists but privkey.pem does not");
            }
            (false, true) => {
                panic!("privkey.pem exists but fullchain.pem does not");
            }
            (true, true) => {}
        }
        Self {
            tls_dir,
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
