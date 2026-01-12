use std::marker::PhantomData;

use askama::Template;

use crate::{
    config::Config,
    services::{Blaze, BlazeProvider, BrokerProvider, Focus, Service, postgres::Postgres},
};

#[derive(Debug, Template)]
#[template(path = "exporter.yml")]
pub struct Exporter<T>
where
    Self: Service,
{
    pub api_key: String,
    host: String,
    db_password: String,
    db_host: String,
    db_name: String,
    db_user: String,
    blaze_host: String,
    project: &'static str,
    deps: PhantomData<T>,
}

impl<T: BrokerProvider + BlazeProvider> Service for Exporter<T> {
    type Dependencies = (Focus<T, Blaze<T>>, Postgres<Self>);

    type ServiceConfig = &'static Config;

    fn from_config(conf: Self::ServiceConfig, (focus, pg): super::Deps<Self>) -> Self {
        let api_key = conf
            .local_conf
            .borrow_mut()
            .generate_secret::<10, Self>("api-key");
        focus.enable_exporter(
            format!("http://{}:8080", Self::service_name()),
            api_key.clone(),
        );
        Self {
            api_key,
            host: conf.hostname.to_string(),
            db_password: pg.password.clone(),
            db_name: pg.db.clone(),
            db_user: pg.user.clone(),
            db_host: <Postgres<Self> as Service>::service_name(),
            blaze_host: <Blaze<T> as Service>::service_name(),
            project: T::network_name(),
            deps: PhantomData,
        }
    }

    fn service_name() -> String {
        format!("{}-exporter", T::network_name())
    }
}
