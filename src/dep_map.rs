use std::{
    any::{Any, TypeId},
    collections::HashMap,
    marker::PhantomData,
    ops::Deref,
};

use solvent::DepGraph;

use crate::{services::Service, Config};

#[derive(Default)]
pub struct ServiceMap(HashMap<TypeId, Box<dyn Service>>);

impl std::fmt::Debug for ServiceMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0.keys()).finish()
    }
}

impl ServiceMap {
    fn get<T: Service + Any>(&self) -> Option<&T> {
        self.0
            .get(&TypeId::of::<T>())
            .map(|v| unsafe { &*(v.as_ref() as *const dyn Service as *const T) })
    }

    pub fn get_mut<T: Service + Any>(&mut self) -> Option<&mut T> {
        self.0
            .get_mut(&TypeId::of::<T>())
            .map(|v| unsafe { &mut *(v.as_mut() as *mut dyn Service as *mut T) })
    }

    fn insert<T: Service + Any>(&mut self, v: T) {
        self.0.insert(TypeId::of::<T>(), Box::new(v));
    }

    pub fn to_compose(&self) -> serde_yaml::Value {
        serde_yaml::Value::Mapping(serde_yaml::Mapping::from_iter([(
            "services".into(),
            serde_yaml::Value::Mapping(self.0.iter().fold(
                serde_yaml::Mapping::new(),
                |mut acc, (_, v)| {
                    acc.extend(match v.to_compose() {
                        serde_yaml::Value::Mapping(m) => m,
                        _ => panic!("Service did not return a mapping"),
                    });
                    acc
                },
            )),
        )]))
    }
}

struct DepEntryInner {
    on_created: Vec<Box<dyn FnOnce(&mut dyn Any)>>,
}

impl std::fmt::Debug for DepEntryInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DepEntryInner")
            .field("on_created_len", &self.on_created.len())
            .finish()
    }
}

pub struct DepEntry<'a, T> {
    service: PhantomData<T>,
    inner: &'a mut DepEntryInner,
}

#[derive(Debug, Default)]
pub struct DepMap {
    post_initializers: HashMap<TypeId, DepEntryInner>,
    constructors: HashMap<TypeId, fn(&Config, &mut ServiceMap) -> Box<dyn Service>>,
    dep_tree: DepGraph<TypeId>,
}

trait BoxService: Service {
    fn construct(conf: &Config, deps: &mut ServiceMap) -> Box<dyn Service>
    where
        Self: Sized + 'static,
    {
        Box::new(Self::from_config(conf, deps))
    }
}

impl<T: Service> BoxService for T {}

type DepTreeRoot = ();

impl DepMap {
    pub fn ensure_installed<T: Any + Service>(&mut self) -> DepEntry<'_, T> {
        let type_id = TypeId::of::<T>();
        self.dep_tree
            .register_dependency(TypeId::of::<DepTreeRoot>(), type_id);
        let depends_on = T::dependecies(self);
        self.dep_tree.register_dependencies(type_id, depends_on);
        self.constructors.entry(type_id).or_insert(T::construct);
        let inner = self
            .post_initializers
            .entry(type_id)
            .or_insert_with(|| DepEntryInner {
                on_created: Vec::new(),
            });

        DepEntry {
            service: PhantomData,
            inner,
        }
    }

    pub fn realize(mut self, conf: &Config) -> ServiceMap {
        let mut realized = ServiceMap::default();
        let dep_tree = std::mem::take(&mut self.dep_tree);
        let dep_tree_iter = dep_tree
            .dependencies_of(&TypeId::of::<DepTreeRoot>())
            .unwrap();

        for dep in dep_tree_iter {
            let dep_id = dep.unwrap();
            if *dep_id == TypeId::of::<DepTreeRoot>() {
                continue;
            }
            let constructor = self.constructors.get(dep_id).unwrap();
            let mut dep = constructor(conf, &mut realized);
            if let Some(post_init) = self.post_initializers.remove(dep_id) {
                for post_init_fn in post_init.on_created {
                    post_init_fn(&mut dep);
                }
            }
            realized.0.insert(*dep_id, dep);
        }
        realized
    }
}

impl<'a, T: Any> DepEntry<'a, T> {
    pub fn with(&mut self, f: impl FnOnce(&mut T) + 'static) -> &mut Self {
        self.inner
            .on_created
            .push(Box::new(move |v| f(v.downcast_mut().expect("Should work"))));
        self
    }
}

