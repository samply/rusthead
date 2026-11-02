use serde::Deserialize;

use crate::{
    config::Config,
    modules::{Module, bbmri::Gbn},
    services::{Blaze, BlazeConfig, BlazeProvider, Focus, FocusConfig, ServiceMap},
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
            fn blaze_config(_conf: &Config) -> Option<BlazeConfig> {
                Some(BlazeConfig {
                    referential_integrity: true,
                    block_cache_size: 128,
                    cql_expr_cache_size: 4,
                    heap_size: Some("256M".to_owned()),
                    ..Default::default()
                })
            }
        }
        struct POP;
        impl BlazeProvider for POP {
            fn balze_service_name() -> String {
                "pop-blaze".into()
            }
            fn blaze_config(conf: &Config) -> Option<BlazeConfig> {
                HAP::blaze_config(conf)
            }
        }
        struct SUEP;
        impl BlazeProvider for SUEP {
            fn balze_service_name() -> String {
                "suep-blaze".into()
            }
            fn blaze_config(conf: &Config) -> Option<BlazeConfig> {
                HAP::blaze_config(conf)
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
