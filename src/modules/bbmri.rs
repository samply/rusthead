use serde::Deserialize;

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
    pub directory_sync: Option<DirectorySyncConfig>,
}

pub struct Bbmri;

impl Module for Bbmri {
    fn install(&self, service_map: &mut crate::services::ServiceMap, conf: &crate::config::Config) {
        let Some(bbmri_conf) = conf.bbmri.as_ref() else {
            return;
        };
        service_map.install::<Blaze<Self>>(conf);
        if bbmri_conf.eric {
            let focus = service_map.install::<Focus<Eric, Blaze<Self>>>(conf);
            focus.tag = "main-bbmri".into();
        }
        if bbmri_conf.gbn {
            let focus = service_map.install::<Focus<Gbn, Blaze<Self>>>(conf);
            focus.tag = "main-bbmri".into();
        }
        if bbmri_conf.directory_sync.is_some() {
            service_map.install::<crate::services::DirectorySync<Self>>(conf);
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
