use std::{cell::RefCell, fs, path::PathBuf, rc::Rc};

use askama::Template;
use bcrypt::DEFAULT_COST;
use serde::{Deserialize, Serialize};

use crate::{config::LocalConf, utils::filters};

use super::Service;

#[derive(Debug, Template)]
#[template(path = "traefik.yml")]
pub struct Traefik {
    tls_dir: PathBuf,
    local_conf: Rc<RefCell<LocalConf>>,
}

impl Traefik {
    // TODO: persist to some local.config.toml or smth maybe with toml_edit
    pub fn add_basic_auth_user(&mut self, middleware_name: String) {
        self.local_conf
            .borrow_mut()
            .basic_auth_users
            .get_or_insert_default()
            .entry(middleware_name)
            .or_insert_with(|| {
                let pw = self.local_conf.borrow().generate_secret::<10>();
                let hash = bcrypt::hash(&pw, DEFAULT_COST).unwrap();
                BasicAuthUser { hash, pw: Some(pw) }
            });
    }
}

impl Service for Traefik {
    type Dependencies = ();

    fn from_config(conf: &crate::Config, _deps: super::Deps<Self>) -> Self {
        let tls_dir = conf.path.join("traefik-tls");
        fs::create_dir_all(&tls_dir).unwrap();
        Self {
            tls_dir,
            local_conf: conf.local_conf.clone(),
        }
    }

    fn service_name() -> String {
        "traefik".into()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BasicAuthUser {
    hash: String,
    pw: Option<String>,
}
