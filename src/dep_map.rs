use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

use crate::services::ToCompose;

#[derive(Default)]
pub struct ServiceMap(HashMap<TypeId, Box<dyn ToCompose>>);

impl std::fmt::Debug for ServiceMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0.keys()).finish()
    }
}

impl ServiceMap {
    pub fn get<T: ToCompose + Any>(&self) -> Option<&T> {
        self.0
            .get(&TypeId::of::<T>())
            .map(|v| unsafe { &*(v.as_ref() as *const dyn ToCompose as *const T) })
    }

    pub fn get_mut<T: ToCompose + Any>(&mut self) -> Option<&mut T> {
        self.0
            .get_mut(&TypeId::of::<T>())
            .map(|v| unsafe { &mut *(v.as_mut() as *mut dyn ToCompose as *mut T) })
    }

    pub fn insert<T: ToCompose + Any>(&mut self, v: T) {
        self.0.insert(TypeId::of::<T>(), Box::new(v));
    }

    pub fn remove<T: ToCompose + Any>(&mut self) -> Option<T> {
        self.0
            .remove(&TypeId::of::<T>())
            .map(|v| unsafe { (Box::into_raw(v) as *mut T).read() })
    }

    pub fn to_compose(&self) -> serde_yaml::Value {
        serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([(
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
        )]))
    }
}
