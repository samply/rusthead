use std::marker::PhantomData;

use crate::{
    dep_map::ServiceMap,
    services::beam::{BeamBrokerKind, BeamProxy, DktkBroker},
    Config,
};

pub trait Module {
    fn enabled(&self, conf: &Config) -> bool;

    fn install(&self, service_map: &mut ServiceMap, conf: &Config);
}

struct ExampleModule<T: BeamBrokerKind>(PhantomData<T>);

impl<T: BeamBrokerKind> ExampleModule<T> {
    const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T: BeamBrokerKind + 'static> Module for ExampleModule<T> {
    fn enabled(&self, _conf: &Config) -> bool {
        true
    }

    fn install(&self, service_map: &mut ServiceMap, conf: &Config) {
        service_map.install::<BeamProxy<T>>(conf);
    }
}

pub const CCP_MODULES: &[&dyn Module] = &[&ExampleModule::<DktkBroker>::new()];
