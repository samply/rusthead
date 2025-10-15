use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fs,
};

use anyhow::Context;
use askama::Template;

use crate::{Config, bridgehead::Bridgehead, modules::Module};

mod postgres;
mod teiler;
pub use teiler::*;
mod transfair;
pub use transfair::*;
mod id_managment;
pub use id_managment::*;
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

    fn get<'services>(services: &'services mut ServiceMap) -> Option<Self::DepRefs<'services>>;
}

macro_rules! service_tuple_option {
    ($($opt_ts:ident),* : $($ts:ident),*) => {
        #[allow(unused, non_snake_case)]
        impl<$($ts: Service,)* $($opt_ts: Service,)*> ServiceTuple for ($($ts,)* $(Option<$opt_ts>,)*) {
            type DepRefs<'t> = ($(&'t mut $ts,)* $(Option<&'t mut $opt_ts>,)*);

            fn get<'services>(services: &'services mut ServiceMap) -> Option<Self::DepRefs<'services>> {
                let [$($ts,)* $($opt_ts,)*] = services.map.get_disjoint_mut([
                    $(&TypeId::of::<$ts>(),)*
                    $(&TypeId::of::<$opt_ts>(),)*
                ]);
                Some((
                    // Required services must be present
                    $((($ts?.as_mut() as &mut dyn Any).downcast_mut::<$ts>().unwrap()),)*
                    // Optional services may be absent
                    $($opt_ts.map(|s| (s.as_mut() as &mut dyn Any).downcast_mut::<$opt_ts>().unwrap()),)*
                ))
            }
        }

        #[allow(unused, non_snake_case)]
        impl<$($ts: DefaultService,)* $($opt_ts: Service,)*> DefaultServiceTuple for ($($ts,)* $(Option<$opt_ts>,)*) {
            fn get_or_create<'services>(services: &'services mut ServiceMap) -> Self::DepRefs<'services> {
                // Ensure all required services are created
                $(
                    let service = $ts::from_default_config(services);
                    services.insert(service);
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

            fn get<'services>(services: &'services mut ServiceMap) -> Option<Self::DepRefs<'services>> {
                let [$($ts,)*] = services.map.get_disjoint_mut([
                    $(&TypeId::of::<$ts>(),)*
                ]);
                // Try to get all services, return None if any is missing
                Some(($(($ts?.as_mut() as &mut dyn Any).downcast_mut::<$ts>().unwrap(),)*))
            }
        }

        #[allow(unused, non_snake_case)]
        impl<$($ts: DefaultService,)*> DefaultServiceTuple for ($($ts,)*) {
            fn get_or_create<'services>(services: &'services mut ServiceMap) -> Self::DepRefs<'services> {
                // Ensure all services are created
                $(
                    let service = $ts::from_default_config(services);
                    services.insert(service);
                )*
                let [$($ts,)*] = services.map.get_disjoint_mut([
                    $(&TypeId::of::<$ts>(),)*
                ]);
                // All services are guaranteed to be created at this point
                ($(($ts.unwrap().as_mut() as &mut dyn Any).downcast_mut::<$ts>().unwrap(),)*)
            }
        }
        option_helper!($($ts),*);
    };
}

pub trait DefaultServiceTuple: ServiceTuple {
    fn get_or_create<'services>(services: &'services mut ServiceMap) -> Self::DepRefs<'services>;
}

trait FromConfig {
    fn from_default_config(conf: &'static Config) -> Self;
}

impl FromConfig for &'static Config {
    fn from_default_config(conf: &'static Config) -> Self {
        conf
    }
}

impl FromConfig for () {
    fn from_default_config(_conf: &Config) -> Self {
        ()
    }
}

pub trait DefaultService: Service {
    fn from_default_config(service_map: &mut ServiceMap) -> Self;
}

impl<T> DefaultService for T
where
    T: Service,
    T::ServiceConfig: FromConfig,
    T::Dependencies: DefaultServiceTuple,
{
    fn from_default_config(service_map: &mut ServiceMap) -> Self {
        let conf = T::ServiceConfig::from_default_config(service_map.config);
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
    map: HashMap<TypeId, Box<dyn ToCompose>>,
    config: &'static Config,
}

impl std::fmt::Debug for ServiceMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.map.keys()).finish()
    }
}

impl ServiceMap {
    pub fn new(config: &'static Config) -> Self {
        Self {
            map: HashMap::new(),
            config,
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn write_all(&self) -> anyhow::Result<()> {
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
        let lockfile = cmd.output()?.stdout;
        fs::write(self.config.path.join("docker-image.lock.yml"), lockfile)?;
        if !pull_cmd.status()?.success() {
            anyhow::bail!("Failed to pull images.");
        }
        Ok(())
    }

    pub fn get_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.map
            .get_mut(&TypeId::of::<T>())
            .and_then(|v| (v.as_mut() as &mut dyn Any).downcast_mut::<T>())
    }

    pub fn contains<T: ToCompose + Any>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<T>())
    }

    #[must_use = "Ensure that the service actually got installed because all its deps were already installed"]
    pub fn install_with_config_cached_deps<T: Service>(
        &mut self,
        conf: T::ServiceConfig,
    ) -> &mut T {
        // Workaround for problem case #3
        // https://smallcultfollowing.com/babysteps/blog/2016/04/27/non-lexical-lifetimes-introduction/
        if self.contains::<T>() {
            return self.get_mut().unwrap();
        }
        let s = T::from_config(conf, T::Dependencies::get(self).unwrap());
        self.insert(s)
    }

    pub fn install_with_config<T>(&mut self, conf: T::ServiceConfig) -> &mut T
    where
        T: Service,
        T::Dependencies: DefaultServiceTuple,
    {
        // Workaround for problem case #3
        // https://smallcultfollowing.com/babysteps/blog/2016/04/27/non-lexical-lifetimes-introduction/
        if self.contains::<T>() {
            return self.get_mut().unwrap();
        }
        let s = T::from_config(conf, T::Dependencies::get_or_create(self));
        self.insert(s)
    }

    fn insert<T: Service>(&mut self, s: T) -> &mut T {
        self.map.insert(TypeId::of::<T>(), Box::new(s));
        self.get_mut().unwrap()
    }

    pub fn install_default<T: DefaultService>(&mut self) -> &mut T {
        // Workaround for problem case #3
        // https://smallcultfollowing.com/babysteps/blog/2016/04/27/non-lexical-lifetimes-introduction/
        if self.contains::<T>() {
            return self.get_mut().unwrap();
        }
        let s = T::from_default_config(self);
        self.insert(s)
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
}
