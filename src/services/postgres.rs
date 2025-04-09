use std::marker::PhantomData;

use askama::Template;

use super::Service;

#[derive(Debug, Template)]
#[template(path = "postgres.yml")]
pub struct Postgres<T>
where
    Self: Service,
{
    r#for: PhantomData<T>,
    pub user: String,
    pub db: String,
    pub password: String,
}

impl<T: Service> Service for Postgres<T> {
    type Dependencies = ();
    type ServiceConfig = crate::Config;

    fn from_config(conf: &Self::ServiceConfig, _deps: super::Deps<Self>) -> Self {
        Self {
            r#for: PhantomData,
            user: <T as Service>::service_name(),
            db: <T as Service>::service_name(),
            password: conf.local_conf.borrow().generate_secret::<10>(),
        }
    }

    fn service_name() -> String {
        format!("{}-db", <T as Service>::service_name())
    }
}
