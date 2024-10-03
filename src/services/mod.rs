use std::any::TypeId;

use crate::{
    dep_map::{DepMap, ServiceMap},
    Config,
};

pub mod beam;
pub mod focus;

pub trait Service {
    fn from_config(conf: &Config, deps: &mut ServiceMap) -> Self
    where
        Self: Sized;

    // TODO: I dont like the "duplication" of calling ensure_installed on every dep and returning
    // the type ids
    fn dependecies(dep_map: &mut DepMap) -> Vec<TypeId>
    where
        Self: Sized;

    // TODO: Always quote secrets but just replace this with jinja templates
    // Acutal issue is $ in pws
    fn to_compose(&self) -> serde_yaml::Value;

    fn type_id() -> TypeId
    where
        Self: 'static + Sized,
    {
        TypeId::of::<Self>()
    }
}

