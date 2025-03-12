use crate::services::{beam::DktkBroker, focus::Focus};

use super::Module;


pub struct CcpDefault;

impl Module for CcpDefault {
    fn enabled(&self, conf: &crate::Config) -> bool {
        conf.ccp.is_some()
    }

    fn install(&self, service_map: &mut crate::dep_map::ServiceMap, conf: &crate::Config) {
        service_map.install::<Focus<DktkBroker>>(conf);
    }
}