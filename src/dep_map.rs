use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fs,
};

use crate::{
    modules::Module,
    services::{Service, ToCompose},
    Config,
};

#[derive(Default)]
pub struct ServiceMap(HashMap<TypeId, Box<dyn ToCompose>>);

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

    pub fn install<T: Service>(&mut self, conf: &Config) -> &mut T {
        T::get_or_create(conf, self);
        self.get_mut().unwrap()
    }

    pub fn install_module<M: Module>(&mut self, m: M, conf: &Config) {
        if m.enabled(conf) {
            m.install(self, conf);
        }
    }

    pub fn write_composables(&self) -> anyhow::Result<()> {
        fs::create_dir_all("services")?;
        for service in self.0.values() {
            fs::write(
                format!("services/{}.yml", service.service_name()),
                service.render()?,
            )?;
        }
        Ok(())
    }
}
