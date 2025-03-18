mod ccp;

use crate::{
    services::ServiceMap, Config
};

pub trait Module {
    fn enabled(&self, conf: &Config) -> bool;

    fn install(&self, service_map: &mut ServiceMap, conf: &Config);
}

impl Module for &dyn Module {
    fn enabled(&self, conf: &Config) -> bool {
        (*self).enabled(conf)
    }

    fn install(&self, service_map: &mut ServiceMap, conf: &Config) {
        (*self).install(service_map, conf);
    }
}

pub const MODULES: &[&dyn Module] = &[&ccp::CcpDefault];