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
    type Dependencies<'s> = ();

    fn from_config(_conf: &crate::config::Config, _deps: super::Deps<'_, Self>) -> Self {
        Self {
            r#for: PhantomData,
            user: <T as Service>::service_name(),
            db: <T as Service>::service_name(),
            password: _conf.local_conf.borrow().generate_secret::<10>(),
        }
    }

    fn service_name() -> String {
        format!("{}-db", <T as Service>::service_name())
    }
}
