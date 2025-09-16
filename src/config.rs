use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    ops::Deref,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

use rand::{Rng, SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize};
use url::{Host, Url};

use crate::{
    modules::{BbmriConfig, CcpConfig},
    services::{BasicAuthUser, Service},
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub site_id: String,
    #[serde(with = "crate::utils::host")]
    pub hostname: Host,
    #[serde(default)]
    pub environment: Environment,
    pub https_proxy_url: Option<Url>,
    pub ccp: Option<CcpConfig>,
    pub bbmri: Option<BbmriConfig>,
    #[serde(skip)]
    pub path: PathBuf,

    #[serde(skip)]
    pub local_conf: RefCell<LocalConf>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    #[default]
    Production,
    Acceptance,
    Test,
}

impl Config {
    pub fn load(path: &PathBuf) -> anyhow::Result<Self> {
        anyhow::ensure!(
            path.is_absolute(),
            "Path to config must be absolute unlike {path:?}"
        );
        let mut conf: Config = toml::from_str(&std::fs::read_to_string(path.join("config.toml"))?)?;
        conf.path = path.clone();
        let local_conf = fs::read_to_string(conf.local_conf_path())
            .ok()
            .and_then(|data| toml::from_str(&data).ok())
            .unwrap_or_else(|| {
                eprintln!("Failed to read local config creating a new one");
                LocalConf::default()
            });
        conf.local_conf = RefCell::new(local_conf);
        Ok(conf)
    }

    pub fn trusted_ca_certs(&self) -> PathBuf {
        let dir = self.path.join("trusted-ca-certs");
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    pub fn local_conf_path(&self) -> PathBuf {
        self.path.join("config.local.toml")
    }

    pub fn write_local_conf(&self) -> anyhow::Result<()> {
        let conf_str = toml::to_string_pretty(self.local_conf.borrow().deref())?;
        fs::write(self.local_conf_path(), conf_str)?;
        fs::write(
            self.path.join(".env"),
            self.local_conf.borrow().to_env()?.as_bytes(),
        )?;
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalConf {
    #[serde(default = "generate_seed")]
    seed: u32,
    pub oidc: Option<HashMap<String, String>>,
    pub basic_auth_users: Option<HashMap<String, BasicAuthUser>>,
    #[serde(skip)]
    pub generated_secrets: HashMap<String, String>,
}

fn generate_seed() -> u32 {
    rand::rng().random()
}

impl Default for LocalConf {
    fn default() -> Self {
        LocalConf {
            seed: generate_seed(),
            oidc: None,
            basic_auth_users: None,
            generated_secrets: Default::default(),
        }
    }
}

impl LocalConf {
    #[must_use]
    pub fn generate_secret<const N: usize, T: Service>(&mut self, name: &str) -> String {
        static RNG: OnceLock<Mutex<StdRng>> = OnceLock::new();
        let mut rng = RNG
            .get_or_init(|| StdRng::seed_from_u64(self.seed as u64).into())
            .lock()
            .unwrap();
        let secret = crate::utils::secret_from_rng::<N>(&mut rng);
        let name = format!(
            "{}_{}",
            <T as Service>::service_name()
                .to_uppercase()
                .replace("-", "_"),
            name.to_uppercase()
        );
        let var = format!("${{{name}}}");
        self.generated_secrets.insert(name, secret);
        var
    }

    pub fn to_env(&self) -> anyhow::Result<String> {
        use std::fmt::Write;
        let mut env = String::from(
            "# This file is auto generated please modify config.toml or config.local.toml instead!\n\n",
        );
        if let Some(ref oidc) = self.oidc {
            for (k, v) in oidc {
                writeln!(&mut env, "OIDC_{}=\"{}\"", k.to_uppercase(), v)?;
            }
        }
        for (k, v) in &self.generated_secrets {
            writeln!(&mut env, "{}=\"{}\"", k, v)?;
        }
        Ok(env)
    }
}
