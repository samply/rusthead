use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    ops::Deref,
    path::PathBuf,
    rc::Rc,
    sync::{Mutex, OnceLock},
};

use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use url::{Host, Url};

use crate::{modules::BbmriConfig, services::BasicAuthUser};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub site_id: String,
    #[serde(with = "crate::utils::host")]
    pub hostname: Host,
    pub https_proxy_url: Option<Url>,
    pub ccp: Option<CcpConfig>,
    pub bbmri: Option<BbmriConfig>,
    #[serde(default = "default_srv_dir")]
    pub srv_dir: PathBuf,
    #[serde(skip)]
    pub path: PathBuf,

    #[serde(skip)]
    pub local_conf: Rc<RefCell<LocalConf>>,
}

fn default_srv_dir() -> PathBuf {
    PathBuf::from("/srv/docker/bridgehead")
}

impl Config {
    pub fn load(path: &PathBuf) -> anyhow::Result<Self> {
        anyhow::ensure!(
            path.is_absolute(),
            "Path to config must be absolute unlike {path:?}"
        );
        let mut conf: Config = toml::from_str(&std::fs::read_to_string(path.join("config.toml"))?)?;
        conf.path = path.clone();
        anyhow::ensure!(
            conf.srv_dir.is_absolute(),
            "srv_path must be absolute unlike {:?}",
            conf.srv_dir
        );
        let local_conf = fs::read_to_string(conf.local_conf_path())
            .ok()
            .and_then(|data| toml::from_str(&data).ok())
            .unwrap_or_else(|| {
                eprintln!("Failed to read local config creating a new one");
                LocalConf::default()
            });
        conf.local_conf = Rc::new(RefCell::new(local_conf));
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
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CcpConfig {
    // TODO
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct LocalConf {
    #[serde(default = "generate_seed")]
    pub seed: u64,
    pub oidc: Option<HashMap<String, String>>,
    pub basic_auth_users: Option<HashMap<String, BasicAuthUser>>,
}

fn generate_seed() -> u64 {
    rand::rng().random()
}

impl LocalConf {
    pub fn generate_secret<const N: usize>(&self) -> String {
        static RNG: OnceLock<Mutex<StdRng>> = OnceLock::new();
        let mut rng = RNG
            .get_or_init(|| StdRng::seed_from_u64(self.seed).into())
            .lock()
            .unwrap();
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                                abcdefghijklmnopqrstuvwxyz\
                                0123456789)(*&^%#@!~";
        (0..N)
            .map(|_| {
                let idx = rng.random_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }
}
