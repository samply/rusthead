mod bbmri;
mod ccp;
mod dnpm;
mod eucaim;
mod napkon;

use crate::{Config, services::ServiceMap};
pub use bbmri::BbmriConfig;
pub use ccp::{CcpConfig, CcpDefault};
pub use dnpm::DnpmConfig;
pub use eucaim::{EucaimConfig, EucaimEndpointType};
pub use napkon::NapkonConfig;

pub trait Module {
    fn install(&self, service_map: &mut ServiceMap, conf: &'static Config);
}

impl Module for &dyn Module {
    fn install(&self, service_map: &mut ServiceMap, conf: &'static Config) {
        (*self).install(service_map, conf);
    }
}

pub const MODULES: &[&dyn Module] = &[
    &ccp::CcpDefault,
    &bbmri::Bbmri,
    &dnpm::Dnpm,
    &eucaim::Eucaim,
    &napkon::Napkon,
];
