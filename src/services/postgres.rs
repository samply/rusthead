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
    user: String,
    password: String,
    realm: String,
}

#[derive(Debug)]
pub struct PgConnectInfo {
    pub host: String,
    pub user: String,
    pub realm: String,
    pub password: String,
}

impl<T> Postgres<T>
where
    Self: Service,
{
    pub fn connect_info(&self) -> PgConnectInfo {
        PgConnectInfo {
            host: Self::service_name(),
            user: self.user.clone(),
            realm: self.realm.clone(),
            password: self.password.clone(),
        }
    }
}

impl<T: Service> Service for Postgres<T> {
    type Dependencies = ();
    type ServiceConfig = &'static crate::Config;

    fn from_config(conf: Self::ServiceConfig, _deps: super::Deps<Self>) -> Self {
        Self {
            r#for: PhantomData,
            user: <T as Service>::service_name(),
            realm: <T as Service>::service_name(),
            password: conf
                .local_conf
                .borrow_mut()
                .generate_secret::<10, Self>("password"),
        }
    }

    fn service_name() -> String {
        format!("{}-db", <T as Service>::service_name())
    }
}
