use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fs,
    path::Path,
};

use anyhow::Context;
use rinja::Template;

use crate::{modules::Module, Config};

mod beam;
pub use beam::*;
mod focus;
pub use focus::*;
mod blaze;
pub use blaze::*;
mod traefik;
pub use traefik::*;

pub type Deps<'a, T> = <<T as Service>::Dependencies<'a> as ServiceTuple<'a>>::DepRefs;

// Could remove 'static bound by using dtolnay's typeid crate for the type map
pub trait Service: ToCompose + 'static {
    type Dependencies<'s>: ServiceTuple<'s>;

    fn from_config(conf: &Config, deps: Deps<'_, Self>) -> Self;

    fn service_name() -> String;

    fn get_or_create<'services>(
        conf: &Config,
        deps: &'services mut ServiceMap,
    ) -> &'services mut Self
    where
        Self: Sized,
    {
        // Workaround for problem case #3
        // https://smallcultfollowing.com/babysteps/blog/2016/04/27/non-lexical-lifetimes-introduction/
        if deps.contains::<Self>() {
            return deps.get_mut().unwrap();
        }
        let this = Self::from_config(conf, Self::Dependencies::get_or_create(conf, deps));
        deps.insert(this);
        deps.get_mut().unwrap()
    }
}

pub trait ServiceTuple<'t> {
    type DepRefs;

    fn get_or_create<'service: 't>(
        conf: &Config,
        services: &'service mut ServiceMap,
    ) -> Self::DepRefs;
}

macro_rules! service_tuple {
    ($($ts:ident),*) => {
        impl<'t, $($ts: Service,)*> ServiceTuple<'t> for ($($ts,)*) {
            type DepRefs = ($(&'t mut $ts,)*);

            #[allow(unused)]
            fn get_or_create<'service: 't>(
                conf: &Config,
                services: &'service mut ServiceMap,
            ) -> Self::DepRefs {
                let mut type_ids: Vec<TypeId> = vec![$(TypeId::of::<$ts>()),*];
                let n = type_ids.len();
                type_ids.dedup();
                assert_eq!(n, type_ids.len(), "Service tuple needs to be disjoint");
                // Safety:
                // This is basically a HashMap::get_many_mut so as long as the types don't overlap,
                // which we check above, this code is sound
                unsafe {
                    ($(
                        $ts::get_or_create(conf, &mut *(services as *mut _)),
                    )*)
                }
            }
        }
    };
}

service_tuple!();
service_tuple!(T1);
service_tuple!(T1, T2);
service_tuple!(T1, T2, T3);
service_tuple!(T1, T2, T3, T4);

pub trait ToCompose {
    fn render(&self) -> anyhow::Result<String>;

    fn service_name(&self) -> String;
}

impl<T: Template + Service> ToCompose for T {
    fn render(&self) -> anyhow::Result<String> {
        Template::render(self)
            .with_context(|| format!("Failed to render {}", std::any::type_name::<T>()))
    }

    fn service_name(&self) -> String {
        <T as Service>::service_name()
    }
}

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
        T::get_or_create(conf, self)
    }

    pub fn install_module<M: Module>(&mut self, m: M, conf: &Config) {
        if m.enabled(conf) {
            m.install(self, conf);
        }
    }

    pub fn write_composables(&self, srv_dir: impl AsRef<Path>) -> anyhow::Result<()> {
        let services_dir = srv_dir.as_ref().join("services");
        fs::create_dir_all(&services_dir)?;
        for service in self.0.values() {
            fs::write(
                services_dir.join(format!("{}.yml", service.service_name())),
                service.render()?,
            )?;
        }
        Ok(())
    }
}
