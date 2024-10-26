use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

use crate::{
    services::{Service, ToCompose},
    Config,
};

#[derive(Default)]
pub struct ServiceMap(pub(crate) HashMap<TypeId, Box<dyn ToCompose>>);

impl std::fmt::Debug for ServiceMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0.keys()).finish()
    }
}

impl ServiceMap {
    pub fn get_mut<T: ToCompose + Any>(&mut self) -> Option<&mut T> {
        self.0
            .get_mut(&TypeId::of::<T>())
            .map(|v| unsafe { &mut *(v.as_mut() as *mut dyn ToCompose as *mut T) })
    }

    pub fn insert<T: ToCompose + Any>(&mut self, v: T) {
        self.0.insert(TypeId::of::<T>(), Box::new(v));
    }

    pub fn contains<T: ToCompose + Any>(&self) -> bool {
        self.0.contains_key(&TypeId::of::<T>())
    }

    pub fn install<T: Service>(&mut self, conf: &Config) {
        T::get_or_create(conf, self);
    }

    pub fn to_compose(&self) -> serde_yaml::Value {
        serde_yaml::Mapping::from_iter([(
            "services".into(),
            serde_yaml::Value::Mapping(self.0.iter().fold(
                serde_yaml::Mapping::new(),
                |mut acc, (_, v)| {
                    match v.to_compose() {
                        serde_yaml::Value::Mapping(m) => acc.extend(m),
                        what => panic!("Service did not return a mapping: {what:?}"),
                    };
                    acc
                },
            )),
        )])
        .into()
    }
}
