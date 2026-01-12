use std::{fs, marker::PhantomData, path::PathBuf};

use askama::Template;
use url::Url;

use crate::{
    config::Config,
    services::{
        BeamAppInfos, BeamConnect, BeamProxy, BrokerProvider, ForwardProxy, OidcClient,
        OidcProvider, PrivateOidcClient, Service,
        beam_connect::LocalTarget,
        postgres::{PgConnectInfo, Postgres},
    },
    utils::filters,
};

#[derive(Debug, Template)]
#[template(path = "datashield.yml")]
pub struct DataShield<T: BrokerProvider> {
    tm_beam: BeamAppInfos,
    tm_pw: String,
    host: String,
    opal_pw: String,
    opal_key_path: PathBuf,
    opal_cert_path: PathBuf,
    pub exporter_password: Option<String>,
    fw_proxy_url: Url,
    db: PgConnectInfo,
    oidc: PrivateOidcClient,
    oidc_admin_group: String,
    deps: PhantomData<T>,
}

impl<T: BrokerProvider> DataShield<T> {
    pub fn opal_host(&self) -> String {
        format!("{}-opal", T::network_name())
    }
}

impl<T: BrokerProvider + OidcProvider> Service for DataShield<T> {
    type Dependencies = (ForwardProxy, Postgres<Self>, BeamProxy<T>, BeamConnect<T>);

    type ServiceConfig = &'static Config;

    fn from_config(
        conf: Self::ServiceConfig,
        (fw_proxy, pg, beam_proxy, beam_connect): super::Deps<Self>,
    ) -> Self {
        beam_connect.add_local_target(LocalTarget::new(
            format!("{}:443", conf.site_id),
            format!("{}-opal:8443", T::network_name()),
            vec![format!("central-ds-orchestrator.{}", T::broker_id())],
        ));

        // Self signed opal cert generation
        let key_path = conf
            .path
            .join(format!("pki/{}-opal.priv.pem", T::network_name()));
        let cert_path = conf.trusted_ca_certs().join("opal-cert.pem");
        if !(key_path.exists() && cert_path.exists()) {
            let keypair =
                rcgen::generate_simple_self_signed([format!("{}-opal", T::network_name())])
                    .expect("Failed to generate opal cert");
            fs::write(&key_path, keypair.signing_key.serialize_pem())
                .expect("Failed to write opal priv key");
            fs::write(&cert_path, keypair.cert.pem()).expect("Failed to write opal cert");
        }

        let tm_beam = beam_proxy.add_service("token-manager");
        let mut local_conf = conf.local_conf.borrow_mut();
        Self {
            fw_proxy_url: fw_proxy.get_url(),
            tm_beam,
            oidc: OidcClient::<T>::add_private_redirect_path(conf, "/opal/*"),
            db: pg.connect_info(),
            exporter_password: None,
            tm_pw: local_conf.generate_secret::<10, Self>("token-manager"),
            opal_pw: local_conf.generate_secret::<10, Self>("opal-admin"),
            oidc_admin_group: T::admin_group(conf),
            host: conf.hostname.to_string(),
            deps: PhantomData,
            opal_key_path: key_path,
            opal_cert_path: cert_path,
        }
    }

    fn service_name() -> String {
        format!("{}-datashield", T::network_name())
    }
}
