use std::{collections::HashMap, marker::PhantomData};

use crate::{
    config::Config,
    modules::CcpDefault,
    services::{OidcClient, PrivateOidcClient},
    utils::capitalize_first_letter,
};

use super::{ForwardProxy, Service, ToCompose, Traefik, postgres::Postgres};
use askama::Template;
use serde::Deserialize;
use url::Url;

#[derive(Debug, Deserialize, Clone)]
pub struct IdManagementConfig {
    // Used by teiler for health checks
    pub upload_apikey: String,
    read_apikey: String,
    central_patientlist_apikey: String,
    controlnumbergenerator_apikey: String,
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
    oidc: PrivateOidcClient,
    oidc_group: String,
    conf: &'static IdManagementConfig,
    local_apikey: String,
    postgres_pw: String,
    fw_proxy_url: Url,
    fw_proxy_name: String,
}

impl Service for IdManagement<CcpDefault> {
    type Dependencies = (Traefik, ForwardProxy, Postgres<Self>);
    type ServiceConfig = (&'static IdManagementConfig, &'static Config);

    fn from_config(
        (idm_conf, conf): Self::ServiceConfig,
        (_traefik, fw_proxy, pg): super::Deps<Self>,
    ) -> Self {
        pg.user = "mainzelliste".into();
        pg.db = "mainzelliste".into();
        Self {
            id: legacy_id_mapping(&conf.site_id),
            hostname: conf.hostname.to_string(),
            conf: idm_conf,
            fw_proxy_url: fw_proxy.get_url(),
            fw_proxy_name: fw_proxy.service_name(),
            oidc: OidcClient::<CcpDefault>::add_private_redirect_path(conf, "/oauth2-idm/callback"),
            oidc_group: format!(
                "DKTK_CCP_{}_Verwalter",
                capitalize_first_letter(&conf.site_id)
            ),
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
