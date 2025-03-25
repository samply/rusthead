use std::{collections::HashMap, path::PathBuf};

use bcrypt::DEFAULT_COST;
use rinja::Template;

use crate::utils::{filters, generate_password};

use super::Service;

#[derive(Debug, Template)]
#[template(path = "traefik.yml")]
pub struct Traefik {
    tls_dir: PathBuf,
    basic_auth_users: HashMap<String, String>,
}

impl Traefik {
    // TODO: persist to some local.config.toml or smth maybe with toml_edit
    pub fn add_basic_auth_user(&mut self, middleware_name: String) {
        let hashed_pw = bcrypt::hash(generate_password::<10>(), DEFAULT_COST).unwrap().replace('$', "$$");
        self.basic_auth_users.insert(middleware_name, hashed_pw);
    }
}

impl Service for Traefik {
    type Dependencies<'s> = ();

    fn from_config(conf: &crate::Config, _deps: super::Deps<'_, Self>) -> Self {
        Self {
            tls_dir: conf.path.join("traefik-tls"),
            basic_auth_users: Default::default(),
        }
    }

    fn service_name() -> String {
        "traefik".into()
    }
}
