use std::{any::Any, collections::HashMap};

use serde::{Deserialize, Serialize};


pub trait Service {
    fn from_config(conf: &toml::Value) -> Self where Self: Sized;

    fn to_compose(&self) -> serde_yaml::Value;
}

#[derive(Debug, Serialize, Deserialize)]
struct BeamProxy {
    broker_url: String,
    app_keys: HashMap<String, String>
}

impl Service for BeamProxy {
    fn from_config(conf: &toml::Value) -> Self where Self: Sized {
        Self { broker_url: conf.as_str().unwrap().to_string(), app_keys: HashMap::new() }
    }

    fn to_compose(&self) -> serde_yaml::Value {
        todo!()
    }
}

impl<T: Service> Service for Box<T> {
    fn from_config(conf: &toml::Value) -> Self where Self: Sized {
        Box::new(T::from_config(conf))
    }

    fn to_compose(&self) -> serde_yaml::Value {
        (**self).to_compose()
    }
}