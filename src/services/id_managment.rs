use std::{collections::HashMap, marker::PhantomData};

use crate::{config::Config, modules::CcpDefault};

use super::{ForwardProxy, Service, ToCompose, Traefik, postgres::Postgres};
use askama::Template;
use serde::Deserialize;
use url::Url;

#[derive(Debug, Deserialize, Clone)]
pub struct IdManagementConfig {
    upload_apikey: String,
    read_apikey: String,
    central_patientlist_apikey: String,
    controlnumbergenerator_apikey: String,
    auth_client_secret: String,
    auth_cookie_secret: String,
    #[serde(default)]
    seeds: HashMap<String, (u32, u32, u32)>,
}

#[derive(Debug, Template)]
#[template(path = "id_management.yml")]
pub struct IdManagement<Project>
where
    Self: Service,
{
    project: PhantomData<Project>,
    id: String,
    hostname: String,
    site_id: String,
    oidc_url: Url,
    conf: IdManagementConfig,
    local_apikey: String,
    postgres_pw: String,
    fw_proxy_url: Url,
    fw_proxy_name: String,
}

impl Service for IdManagement<CcpDefault> {
    type Dependencies = (Traefik, ForwardProxy, Postgres<Self>);
    type ServiceConfig = Config;

    fn from_config(conf: &Config, (_traefik, fw_proxy, pg): super::Deps<Self>) -> Self {
        pg.user = "mainzelliste".into();
        pg.db = "mainzelliste".into();
        Self {
            id: legacy_id_mapping(&conf.site_id),
            hostname: conf.hostname.to_string(),
            site_id: conf.site_id.clone(),
            conf: conf.ccp.as_ref().unwrap().id_manager.clone().unwrap(),
            fw_proxy_url: fw_proxy.get_url(),
            fw_proxy_name: fw_proxy.service_name(),
            oidc_url: "https://login.verbis.dkfz.de/realms/master"
                .parse()
                .unwrap(),
            project: PhantomData,
            postgres_pw: pg.password.clone(),
            local_apikey: conf.local_conf.borrow().generate_secret::<10>(),
        }
    }

    fn service_name() -> String {
        "ccp-id-management".into()
    }
}

impl<T> IdManagement<T>
where
    Self: Service,
{
    pub fn pg_name() -> String {
        <Postgres<Self> as Service>::service_name()
    }
}

fn legacy_id_mapping(site_id: &str) -> String {
    site_id
        .split('-')
        .map(capitalize_first_letter)
        .collect::<Vec<_>>()
        .join(" ")
        .replace("Tum", "TUM")
        .replace("Lmu", "LMU")
        .replace("Dktk Test", "Teststandort")
        .replace(" ", "")
}

fn capitalize_first_letter(s: &str) -> String {
    let mut chars = s.chars();
    chars
        .next()
        .map(char::to_uppercase)
        .into_iter()
        .flatten()
        .chain(chars)
        .collect()
}
