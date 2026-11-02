use serde::Deserialize;

use crate::{
    config::Config,
    modules::{Module, bbmri::Gbn},
    services::{Blaze, BlazeProvider, Focus, FocusConfig, ServiceMap},
};

#[derive(Debug, Deserialize)]
pub struct NapkonConfig {}

pub struct Napkon;

impl Module for Napkon {
    fn install(&self, service_map: &mut ServiceMap, conf: &'static Config) {
        let Some(NapkonConfig {}) = conf.napkon else {
            return;
        };
        struct HAP;
        impl BlazeProvider for HAP {
            fn balze_service_name() -> String {
                "hap-blaze".into()
            }
        }
        struct POP;
        impl BlazeProvider for POP {
            fn balze_service_name() -> String {
                "pop-blaze".into()
            }
        }
        struct SUEP;
        impl BlazeProvider for SUEP {
            fn balze_service_name() -> String {
                "suep-blaze".into()
            }
        }
        service_map.install_with_config::<Focus<Gbn, Blaze<HAP>>>(FocusConfig {
            tag: "develop".to_owned(),
            app_name: "hap-focus",
        });
        service_map.install_with_config::<Focus<Gbn, Blaze<POP>>>(FocusConfig {
            tag: "develop".to_owned(),
            app_name: "pop-focus",
        });
        service_map.install_with_config::<Focus<Gbn, Blaze<SUEP>>>(FocusConfig {
            tag: "develop".to_owned(),
            app_name: "suep-focus",
        });
    }
}
