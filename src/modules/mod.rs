mod bbmri;
mod ccp;

pub use bbmri::BbmriConfig;
pub use ccp::{CcpConfig, CcpDefault};
use crate::{services::ServiceMap, Config};

pub trait Module {
    fn install(&self, service_map: &mut ServiceMap, conf: &Config);
}

impl Module for &dyn Module {
    fn install(&self, service_map: &mut ServiceMap, conf: &Config) {
        (*self).install(service_map, conf);
    }
}

pub const MODULES: &[&dyn Module] = &[&ccp::CcpDefault, &bbmri::Bbmri];
