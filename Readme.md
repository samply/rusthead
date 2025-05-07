
# Rusthead (someone please think of a better name)

A tool for generating a number of docker compose files based on some configuration taking into account dependency resolution and deduplication.

## Adding a service

1. Add a file to `src/services` and add it to the `mod.rs`.
2. Create a struct with all parameters for the service.
3. Implement the `Service` trait for your struct. The trait expects two associated types. `Dependencies` is always a tuple of things that implement `Service` (`()` for no deps or `(Service1,)` for a single dependency). `ServiceConfig` is can be a `&'static Config` if your service does not need any specific configuration but you can set it to your custom type. You will get mutable references to your dependencies in the `from_config` method which you can use to construct your service. For the `service_name` method it is important to generate a unique name especially if your service is generic! You need to make sure it generates different service names for different generic parameters in order to prevent name collisions in the generated docker compose files. See [service example](#service-example).
4. Derive the `Template` trait and add a template to `templates/`. See the [example](#template-example) for more details.
5. For your service to be loaded it needs to be installed by a `Module` as described [here](#adding-a-module).


### Service Example:
#### Service implementation example:
`src/services/my_serive.rs`:
```rs
#[derive(Template)]
#[template("my_serive.yml")]
struct MyService {
    some_prop: String,
}

impl Service for MyService {
    type Dependencies = (Traefik,);
    type ServiceConfig = &'static Config;

    fn from_config(_conf: Self::ServiceConfig, _deps: super::Deps<Self>) -> Self {
        Self { some_prop: "foo".into() }
    }

    fn service_name() -> String {
        "my-service".into()
    }
}
```
#### Template example:
`tepmplates/my_serive.yml`:
```yml
services:
  {{ Self::service_name() }}:
    image: my_image
    environment:
      A: { some_prop }
```

## Adding a module

1. Add a file to `src/modules` and add it to the `mod.rs`.
2. Create a struct (must be constructable in const context (most likely a unit struct is enough)) and implement `Module` for it.
3. In the `install` method you can use the `service_map`s install method to add a service. This will return you a mutable reference to the service for further modification if necessary. See the [Example](#module-example).
4. Add the services to the `MODULES` const in `src/modules/mod.rs`.

### Module Example

`src/services/my_serive.rs`:
```rs
pub struct MyModule;

impl Module for MyModule {
    fn enabled(&self, conf: &crate::Config) -> bool {
        conf.ccp.is_some()
    }

    fn install(&self, service_map: &mut crate::dep_map::ServiceMap, conf: &crate::Config) {
        service_map.install::<MyService>(conf);
    }
}
```