use std::any::TypeId;

use crate::{dep_map::ServiceMap, Config};

pub mod beam;
pub mod focus;

pub trait Service {
    fn from_config(&self, conf: &Config, deps: &mut ServiceMap) -> Box<dyn ToCompose>;

    fn dependecies(&self) -> Vec<TypeId>;
}

impl<D1, S> Service for fn(&Config, &mut D1) -> S
where
    D1: ToCompose + 'static,
    S: ToCompose + 'static,
{
    fn from_config(&self, conf: &Config, deps: &mut ServiceMap) -> Box<dyn ToCompose> {
        Box::new(self(conf, deps.get_mut().unwrap()))
    }

    fn dependecies(&self) -> Vec<TypeId> {
        vec![std::any::TypeId::of::<D1>()]
    }
}

impl<S: ToCompose + 'static> Service for fn(&Config) -> S {
    fn from_config(&self, conf: &Config, _deps: &mut ServiceMap) -> Box<dyn ToCompose> {
        Box::new(self(conf))
    }

    fn dependecies(&self) -> Vec<TypeId> {
        Vec::new()
    }
}

pub trait ToCompose {
    // TODO: Always quote secrets but just replace this with jinja templates
    // Acutal issue is $ in pws
    fn to_compose(&self) -> serde_yaml::Value;
}

