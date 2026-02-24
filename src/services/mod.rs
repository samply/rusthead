use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fs,
    marker::PhantomData,
};

use anyhow::Context;
use askama::Template;

use crate::{Config, bridgehead::Bridgehead, modules::Module};

pub mod beam_connect;
pub mod dnpm_node;
pub mod obds2fhir;
pub use beam_connect::BeamConnect;
mod datashield;
pub use datashield::DataShield;
mod exporter;
pub use exporter::Exporter;
mod postgres;
mod teiler;
pub use teiler::*;
mod transfair;
pub use transfair::*;
mod id_management;
pub use id_management::*;
mod directory_sync;
pub use directory_sync::*;
mod forward_proxy;
pub use forward_proxy::ForwardProxy;
mod secret_sync;
pub use secret_sync::*;
mod beam;
pub use beam::*;
mod focus;
pub use focus::*;
mod blaze;
pub use blaze::*;
mod traefik;
pub use traefik::*;

pub type Deps<'a, T> = <<T as Service>::Dependencies as ServiceTuple>::DepRefs<'a>;

// Could remove 'static bound by using dtolnay's typeid crate for the type map
pub trait Service: ToCompose + 'static {
    type Dependencies: ServiceTuple;
    type ServiceConfig;

    fn from_config(conf: Self::ServiceConfig, deps: Deps<Self>) -> Self;

    fn service_name() -> String;
}

pub trait ServiceTuple {
    type DepRefs<'t>;

    fn get_or_create<'services>(services: &'services mut ServiceMap) -> Self::DepRefs<'services>;

    fn register_deps(parent: TypeId, deps: &mut solvent::DepGraph<TypeId>);
}

macro_rules! service_tuple_option {
    ($($opt_ts:ident),* : $($ts:ident),*) => {
        #[allow(unused, non_snake_case)]
        impl<$($ts: Service,)* $($opt_ts: Service,)*> ServiceTuple for ($($ts,)* $(Option<$opt_ts>,)*) {
            type DepRefs<'t> = ($(&'t mut $ts,)* $(Option<&'t mut $opt_ts>,)*);

            fn get_or_create<'services>(services: &'services mut ServiceMap) -> Self::DepRefs<'services> {
                // Ensure all required services are created
                $(
                    if !services.contains::<$ts>() {
                        let service = $ts::from_default_config(services);
                        services.insert(service);
                    }
                )*
                let [$($ts,)* $($opt_ts,)*] = services.map.get_disjoint_mut([
                    $(&TypeId::of::<$ts>(),)*
                    $(&TypeId::of::<$opt_ts>(),)*
                ]);
                (
                    // All required services are guaranteed to be created at this point
                    $((($ts.unwrap().as_mut() as &mut dyn Any).downcast_mut::<$ts>().unwrap()),)*
                    // Optional services may be absent
                    $($opt_ts.map(|s| (s.as_mut() as &mut dyn Any).downcast_mut::<$opt_ts>().unwrap()),)*
                )
            }

            fn register_deps(parent: TypeId, deps: &mut solvent::DepGraph<TypeId>) {
                deps.register_dependencies(parent, vec![$(TypeId::of::<$ts>(),)* $(TypeId::of::<$opt_ts>(),)*]);
                $(
                    $ts::Dependencies::register_deps(TypeId::of::<$ts>(), deps);
                )*
                $(
                    $opt_ts::Dependencies::register_deps(TypeId::of::<$opt_ts>(), deps);
                )*
            }
        }
    };
}

macro_rules! option_helper {
    ($opt_ts:ident $(,)? $($ts:ident),*) => {
        service_tuple_option!($opt_ts : $($ts),*);
        option_helper!([$opt_ts], $($ts),*);
    };
    ([$($opt_ts:ident),*], $new_opt_ts:ident, $($ts:ident),+) => {
        service_tuple_option!($($opt_ts,)* $new_opt_ts : $($ts),*);
        option_helper!([$($opt_ts,)* $new_opt_ts], $($ts),*);
    };
    ([$($opt_ts:ident),*], $new_opt_ts:ident) => {
        service_tuple_option!($($opt_ts,)* $new_opt_ts :);
    };
    ([$($opt_ts:ident),*],) => {};
    () => {};
}

macro_rules! service_tuple {
    ($($ts:ident),*) => {
        #[allow(unused, non_snake_case)]
        impl<$($ts: Service,)*> ServiceTuple for ($($ts,)*) {
            type DepRefs<'t> = ($(&'t mut $ts,)*);

            fn get_or_create<'services>(services: &'services mut ServiceMap) -> Self::DepRefs<'services> {
                // Ensure all services are created
                $(
                    if !services.contains::<$ts>() {
                        let service = $ts::from_default_config(services);
                        services.insert(service);
                    }
                )*
                let [$($ts,)*] = services.map.get_disjoint_mut([
                    $(&TypeId::of::<$ts>(),)*
                ]);
                // All services are guaranteed to be created at this point
                ($(($ts.unwrap().as_mut() as &mut dyn Any).downcast_mut::<$ts>().unwrap(),)*)
            }

            fn register_deps(parent: TypeId, deps: &mut solvent::DepGraph<TypeId>) {
                deps.register_dependencies(parent, vec![$(TypeId::of::<$ts>(),)*]);
                $(
                    $ts::Dependencies::register_deps(TypeId::of::<$ts>(), deps);
                )*
            }
        }

        option_helper!($($ts),*);
    };
}

pub trait DefaultService: Service {
    fn from_default_config(service_map: &mut ServiceMap) -> Self;
}

impl<T> DefaultService for T
where
    T: Service,
    T::ServiceConfig: 'static,
{
    fn from_default_config(service_map: &mut ServiceMap) -> Self {
        let conf: T::ServiceConfig =
            if TypeId::of::<T::ServiceConfig>() == TypeId::of::<&'static Config>() {
                unsafe { std::mem::transmute_copy::<&Config, _>(&service_map.config) }
            } else if TypeId::of::<T::ServiceConfig>() == TypeId::of::<()>() {
                unsafe { std::mem::transmute_copy(&()) }
            } else {
                panic!(
                    "Cannot create {} because it needs to be constructed explicitly with {}",
                    std::any::type_name::<T>(),
                    std::any::type_name::<T::ServiceConfig>()
                );
            };
        let deps = T::Dependencies::get_or_create(service_map);
        T::from_config(conf, deps)
    }
}

service_tuple!();
service_tuple!(T1);
service_tuple!(T1, T2);
service_tuple!(T1, T2, T3);
service_tuple!(T1, T2, T3, T4);

pub trait ToCompose: Any {
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

pub struct ServiceMap {
    deps: solvent::DepGraph<TypeId>,
    constructors: HashMap<TypeId, Box<dyn FnOnce(&mut Self) -> Box<dyn ToCompose>>>,
    post_install: HashMap<TypeId, Vec<Box<dyn FnOnce(&mut dyn ToCompose)>>>,
    map: HashMap<TypeId, Box<dyn ToCompose>>,
    config: &'static Config,
}

pub struct PostInstallBuilder<'a, T>(&'a mut ServiceMap, PhantomData<T>);

impl<T: Service> PostInstallBuilder<'_, T> {
    pub fn post_install(self, post_install: impl FnOnce(&mut T) + 'static) -> Self {
        self.0
            .post_install
            .entry(TypeId::of::<T>())
            .or_default()
            .push(Box::new(move |service| {
                post_install((service as &mut dyn Any).downcast_mut::<T>().unwrap())
            }));
        self
    }
}

impl std::fmt::Debug for ServiceMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.map.keys()).finish()
    }
}

impl ServiceMap {
    const ROOT_NODE: TypeId = TypeId::of::<Bridgehead>();

    pub fn new(config: &'static Config) -> Self {
        let mut deps = solvent::DepGraph::new();
        deps.register_node(Self::ROOT_NODE);
        Self {
            deps,
            constructors: HashMap::new(),
            post_install: HashMap::new(),
            map: HashMap::new(),
            config,
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn write_all(&mut self) -> anyhow::Result<()> {
        self.materialize();
        self.write_composables()
            .context("Failed to write services")?;
        Bridgehead::new(self.config).write()?;
        self.config.write_local_conf()?;
        fs::write(
            self.config.path.join(".gitignore"),
            include_str!("../../static/.gitignore"),
        )?;
        #[cfg(not(test))]
        self.generate_lockfile_and_pull()
            .context("Failed to generate lockfile and pull images")?;
        Ok(())
    }

    #[cfg(not(test))]
    fn generate_lockfile_and_pull(&self) -> anyhow::Result<()> {
        if self.map.is_empty() {
            return Ok(());
        }
        use std::process::Command;
        let mut cmd = Command::new("docker-compose");
        let mut pull_cmd = Command::new("docker-compose");
        for service in self.map.values() {
            let path = self
                .config
                .path
                .join("services")
                .join(format!("{}.yml", service.service_name()));
            cmd.arg("-f").arg(&path);
            pull_cmd.arg("-f").arg(&path);
        }
        if fs::exists(self.config.path.join("docker-compose.override.yml"))? {
            cmd.arg("-f").arg("docker-compose.override.yml");
            pull_cmd.arg("-f").arg("docker-compose.override.yml");
        }
        cmd.args(["--env-file", ".env", "config", "--lock-image-digests"])
            .current_dir(&self.config.path);
        pull_cmd
            .args(["--env-file", ".env", "pull", "--quiet"])
            .current_dir(&self.config.path);
        let output = cmd.output()?;
        if !output.status.success() {
            anyhow::bail!(
                "Failed to generate lockfile: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        fs::write(
            self.config.path.join("docker-image.lock.yml"),
            output.stdout,
        )?;
        if !pull_cmd.status()?.success() {
            anyhow::bail!(
                "Failed to pull images: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }

    pub fn contains<T: ToCompose + Any>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<T>())
    }

    pub fn install_with_config<T: Service>(
        &mut self,
        conf: T::ServiceConfig,
    ) -> PostInstallBuilder<'_, T> {
        self.deps
            .register_dependency(Self::ROOT_NODE, TypeId::of::<T>());
        T::Dependencies::register_deps(TypeId::of::<T>(), &mut self.deps);
        self.constructors.insert(
            TypeId::of::<T>(),
            Box::new(|s| Box::new(T::from_config(conf, T::Dependencies::get_or_create(s)))),
        );
        PostInstallBuilder(self, PhantomData)
    }

    pub fn install_default<T: DefaultService>(&mut self) -> PostInstallBuilder<'_, T> {
        self.deps
            .register_dependency(Self::ROOT_NODE, TypeId::of::<T>());
        T::Dependencies::register_deps(TypeId::of::<T>(), &mut self.deps);
        self.constructors.insert(
            TypeId::of::<T>(),
            Box::new(|s| Box::new(T::from_default_config(s))),
        );
        PostInstallBuilder(self, PhantomData)
    }

    fn insert<T: Service>(&mut self, s: T) {
        self.map.insert(TypeId::of::<T>(), Box::new(s));
    }

    pub fn install_module<M: Module>(&mut self, m: M) {
        m.install(self, &self.config);
    }

    fn write_composables(&self) -> anyhow::Result<()> {
        let services_dir = self.config.path.join("services");
        _ = fs::remove_dir_all(&services_dir);
        fs::create_dir_all(&services_dir)?;
        for service in self.map.values() {
            let service_name = service.service_name();
            eprintln!("Generating service {service_name}");
            fs::write(
                services_dir.join(format!("{}.yml", service.service_name())),
                service.render()?,
            )?;
        }
        Ok(())
    }

    fn materialize(&mut self) {
        let deps = std::mem::take(&mut self.deps);
        for dep in deps.dependencies_of(&Self::ROOT_NODE).unwrap() {
            let dep = dep.expect("No cycle");
            if self.map.contains_key(dep) {
                continue;
            }
            let Some(c) = self.constructors.remove(dep) else {
                // We assume that this would be an optional dependency in this case
                continue;
            };
            let mut service = c(self);
            if let Some(post_install) = self.post_install.remove(dep) {
                for post in post_install {
                    post(service.as_mut());
                }
            }
            self.map.insert(dep.clone(), service);
        }
    }
}
