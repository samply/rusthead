use serde::Deserialize;

use crate::config::Environment;
use crate::services::{Blaze, BlazeProvider, BrokerProvider, DirectorySyncConfig, Focus};
use crate::utils::enabled;

use super::Module;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BbmriConfig {
    #[serde(default = "enabled")]
    eric: bool,
    #[serde(default)]
    gbn: bool,
    directory_sync: Option<DirectorySyncConfig>,
}

pub struct Bbmri;

impl Module for Bbmri {
    fn install(
        &self,
        service_map: &mut crate::services::ServiceMap,
        conf: &'static crate::config::Config,
    ) {
        let Some(bbmri_conf) = conf.bbmri.as_ref() else {
            return;
        };
        service_map.install_default::<Blaze<Self>>();
        if bbmri_conf.eric {
            if let Environment::Acceptance = conf.environment {
                service_map.install_with_config::<Focus<EricAcc, Blaze<Self>>>("main-bbmri".into());
            } else {
                service_map.install_with_config::<Focus<Eric, Blaze<Self>>>("main-bbmri".into());
            }
        }
        if bbmri_conf.gbn {
            service_map.install_with_config::<Focus<Gbn, Blaze<Self>>>("main-bbmri".into());
        }
        if let Some(ds_conf) = &bbmri_conf.directory_sync {
            service_map.install_with_config::<crate::services::DirectorySync<Self>>(ds_conf);
        }
    }
}

impl BlazeProvider for Bbmri {
    fn balze_service_name() -> String {
        "bbmri-blaze".to_owned()
    }

    fn treafik_exposure() -> Option<crate::services::BlazeTraefikConfig> {
        Some(crate::services::BlazeTraefikConfig {
            middleware_and_user_name: "bbmri-blaze".to_owned(),
            path: "/bbmri-localdatamanagement".to_owned(),
        })
    }
}

struct Eric;

impl BrokerProvider for Eric {
    fn broker_url() -> url::Url {
        "https://broker.bbmri.samply.de".parse().unwrap()
    }

    fn network_name() -> &'static str {
        "eric"
    }

    fn root_cert() -> &'static str {
        include_str!("../../static/beam/eric.root.crt.pem")
    }
}

struct Gbn;

impl BrokerProvider for Gbn {
    fn broker_url() -> url::Url {
        "https://broker.bbmri.de".parse().unwrap()
    }

    fn network_name() -> &'static str {
        "gbn"
    }

    fn root_cert() -> &'static str {
        include_str!("../../static/beam/gbn.root.crt.pem")
    }
}

struct EricAcc;

impl BrokerProvider for EricAcc {
    fn broker_url() -> url::Url {
        "https://broker-acc.bbmri-acc.samply.de".parse().unwrap()
    }

    fn network_name() -> &'static str {
        "eric-acc"
    }

    fn root_cert() -> &'static str {
        include_str!("../../static/beam/bbmri.acc.root.crt.pem")
    }
}
