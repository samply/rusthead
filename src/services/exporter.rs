use std::marker::PhantomData;

use askama::Template;

use crate::{
    config::Config,
    services::{
        Blaze, BlazeProvider, BrokerProvider, Focus, OidcProvider, Service,
        datashield::DataShield,
        postgres::{PgConnectInfo, Postgres},
    },
};

#[derive(Debug, Template)]
#[template(path = "exporter.yml")]
pub struct Exporter<T>
where
    Self: Service,
{
    pub api_key: String,
    host: String,
    opal_password: Option<String>,
    db: PgConnectInfo,
    blaze_host: String,
    project: &'static str,
    deps: PhantomData<T>,
}

impl<T: BrokerProvider + BlazeProvider + OidcProvider> Service for Exporter<T> {
    type Dependencies = (Focus<T, Blaze<T>>, Postgres<Self>, Option<DataShield<T>>);

    type ServiceConfig = &'static Config;

    fn from_config(conf: Self::ServiceConfig, (focus, pg, ds): super::Deps<Self>) -> Self {
        let api_key = conf
            .local_conf
            .borrow_mut()
            .generate_secret::<10, Self>("api-key");
        focus.enable_exporter(
            format!("http://{}:8080", Self::service_name()),
            api_key.clone(),
        );
        let opal_password = if let Some(ds) = ds {
            let opal_pw = conf
                .local_conf
                .borrow_mut()
                .generate_secret::<10, Self>("opal-pw");
            ds.exporter_password = Some(opal_pw.clone());
            Some(opal_pw)
        } else {
            None
        };
        Self {
            api_key,
            host: conf.hostname.to_string(),
            opal_password,
            db: pg.connect_info(),
            blaze_host: <Blaze<T> as Service>::service_name(),
            project: T::network_name(),
            deps: PhantomData,
        }
    }

    fn service_name() -> String {
        format!("{}-exporter", T::network_name())
    }
}
