use std::marker::PhantomData;

use askama::Template;
use serde::Deserialize;
use url::Url;

use crate::{
    config::Config, modules::CcpDefault, services::ForwardProxy, utils::capitalize_first_letter,
};

use super::{BrokerProvider, OidcClient, PublicOidcClient, Service};

#[derive(Debug, Deserialize)]
pub struct TeilerConfig {
    #[serde(default = "default_language")]
    language: String,
}

fn default_language() -> String {
    "DE".to_string()
}

#[derive(Template)]
#[template(path = "teiler.yml")]
pub struct Teiler<T>
where
    Self: Service,
{
    project_t: PhantomData<T>,
    oidc_client: PublicOidcClient,
    conf: &'static TeilerConfig,
    exporter_api_key: String,
    project: String,
    mtba_enabled: bool,
    datashield_enabled: bool,
    idm_upload_apikey: Option<String>,
    forward_proxy_url: Url,
    oidc_user_group: String,
    oidc_admin_group: String,
}

impl Service for Teiler<CcpDefault> {
    type Dependencies = (ForwardProxy,);

    type ServiceConfig = (&'static TeilerConfig, &'static Config);

    fn from_config(
        (conf, global_conf): Self::ServiceConfig,
        (fw_proxy,): super::Deps<Self>,
    ) -> Self {
        Self {
            project_t: PhantomData,
            oidc_client: OidcClient::<CcpDefault>::add_public_redirect_path(
                global_conf,
                &format!("/{}", Self::service_name()),
            ),
            project: "ccp".to_string(),
            conf,
            forward_proxy_url: fw_proxy.get_url(),
            exporter_api_key: global_conf.local_conf.borrow().generate_secret::<10>(),
            mtba_enabled: false,
            datashield_enabled: global_conf
                .ccp
                .as_ref()
                .is_some_and(|c| c.datashield.is_some()),
            idm_upload_apikey: global_conf
                .ccp
                .as_ref()
                .and_then(|c| c.id_manager.as_ref())
                .map(|idm| idm.upload_apikey.clone()),
            oidc_user_group: format!("DKTK_CCP_{}", capitalize_first_letter(&global_conf.site_id)),
            oidc_admin_group: format!(
                "DKTK_CCP_{}_Verwalter",
                capitalize_first_letter(&global_conf.site_id)
            ),
        }
    }

    fn service_name() -> String {
        format!("{}-teiler", CcpDefault::network_name())
    }
}
