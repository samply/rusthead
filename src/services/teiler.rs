use std::marker::PhantomData;

use askama::Template;
use serde::Deserialize;
use url::Url;

use crate::{
    config::Config,
    modules::CcpDefault,
    services::{Exporter, ForwardProxy, IdManagement},
    utils::capitalize_first_letter,
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
    /// Exporter host and API key
    exporter: Option<(String, String)>,
    project: String,
    mtba_enabled: bool,
    datashield_enabled: bool,
    idm_upload_apikey: Option<String>,
    forward_proxy_url: Url,
    oidc_user_group: String,
    oidc_admin_group: String,
}

impl Service for Teiler<CcpDefault> {
    type Dependencies = (
        ForwardProxy,
        Option<IdManagement<CcpDefault>>,
        Option<Exporter<CcpDefault>>,
    );

    type ServiceConfig = (&'static TeilerConfig, &'static Config);

    fn from_config(
        (conf, global_conf): Self::ServiceConfig,
        (fw_proxy, idm, exporter): super::Deps<Self>,
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
            exporter: exporter.map(|e| (Exporter::<CcpDefault>::service_name(), e.api_key.clone())),
            mtba_enabled: false,
            datashield_enabled: global_conf
                .ccp
                .as_ref()
                .is_some_and(|c| c.datashield.is_some()),
            idm_upload_apikey: idm.map(|idm| idm.conf.upload_apikey.clone()),
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
