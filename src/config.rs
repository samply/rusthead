use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs,
    ops::Deref,
    path::PathBuf,
};

use anyhow::Context;
use rand::{RngExt, SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize};
use url::{Host, Url};

use crate::{
    modules::{BbmriConfig, CcpConfig, DnpmConfig, EucaimConfig},
    services::{BasicAuthUser, Service, TraefikConfig},
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub site_id: String,
    #[serde(with = "crate::utils::host")]
    pub hostname: Host,
    #[serde(default)]
    pub environment: Environment,
    /// Rusthead Docker image to use (defaults to "samply/rusthead:latest")
    #[serde(default = "default_image")]
    pub image: String,
    /// Defaults to docker named volumes
    pub volume_dir: Option<PathBuf>,
    /// Explicitly enable upstream synchronization during updates.
    #[serde(default)]
    pub git_sync: bool,
    pub https_proxy_url: Option<Url>,
    pub ccp: Option<CcpConfig>,
    pub bbmri: Option<BbmriConfig>,
    pub dnpm: Option<DnpmConfig>,
    pub eucaim: Option<EucaimConfig>,
    pub traefik: Option<TraefikConfig>,
    /// Path to the folder in which this config.toml was located
    #[serde(skip)]
    pub path: PathBuf,

    #[serde(skip)]
    pub local_conf: RefCell<LocalConf>,
}

fn default_image() -> String {
    "samply/rusthead:latest".to_string()
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
        let file = if path.is_dir() {
            path.join("config.toml")
        } else {
            path.clone()
        };
        let mut conf: Config = toml::from_str(&std::fs::read_to_string(&file)?)?;
        conf.path = file.parent().unwrap().to_path_buf();
        let mut local_conf: LocalConf = match fs::read_to_string(conf.local_conf_path()) {
            Ok(data) => toml::from_str(&data).context("Failed to parse config.local.toml")?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => LocalConf::default(),
            Err(error) => return Err(error).context("Failed to read config.local.toml"),
        };
        if !conf
            .path
            .join(format!("pki/{}.priv.pem", conf.site_id))
            .try_exists()?
        {
            local_conf.enrolled_beam_networks.clear();
        }
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
        self.save_local_conf()?;
        fs::write(
            self.path.join(".env"),
            self.local_conf.borrow().to_env()?.as_bytes(),
        )?;
        Ok(())
    }

    /// Persist enrollment progress without rewriting the generated environment.
    pub fn save_local_conf(&self) -> anyhow::Result<()> {
        let conf_str = toml::to_string_pretty(self.local_conf.borrow().deref())?;
        fs::write(self.local_conf_path(), conf_str)?;
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalConf {
    #[serde(default = "generate_seed")]
    seed: u32,
    pub oidc: Option<BTreeMap<String, String>>,
    pub basic_auth_users: Option<BTreeMap<String, BasicAuthUser>>,
    /// Networks required by the current service configuration.
    #[serde(default)]
    pub beam_networks: BTreeSet<String>,
    /// Networks for which the local enrollment command completed successfully.
    /// Retained when a network is disabled so re-enabling it does not re-enroll it.
    #[serde(default)]
    pub enrolled_beam_networks: BTreeSet<String>,
    #[serde(skip)]
    pub generated_secrets: BTreeMap<String, String>,
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
            beam_networks: Default::default(),
            enrolled_beam_networks: Default::default(),
            generated_secrets: Default::default(),
        }
    }
}

impl LocalConf {
    pub fn pending_beam_networks(&self) -> BTreeSet<String> {
        self.beam_networks
            .difference(&self.enrolled_beam_networks)
            .cloned()
            .collect()
    }

    #[must_use]
    pub fn generate_secret<const N: usize, T: Service>(&mut self, name: &str) -> String {
        let name = format!(
            "{}_{}",
            <T as Service>::service_name().to_uppercase(),
            name.to_uppercase()
        )
        .replace("-", "_");
        let salt = name
            .chars()
            .fold(0_u64, |a, b| a.wrapping_mul(31).wrapping_add(b as u64));
        let mut rng = StdRng::seed_from_u64(self.seed as u64 ^ salt);
        let secret = crate::utils::secret_from_rng::<N>(&mut rng);
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

#[cfg(test)]
mod tests {
    use crate::{modules, services::ServiceMap};

    use super::*;

    #[test]
    fn test_configs() {
        let mut s = insta::Settings::clone_current();
        s.set_prepend_module_to_snapshot(false);
        let _guard = s.bind_to_scope();
        insta::glob!("../tests/configs", "*.toml", |conf_path| {
            let temp_dir = tempfile::tempdir().unwrap();
            fs::copy(conf_path, temp_dir.path().join("config.toml")).unwrap();
            let conf = Config::load(&temp_dir.path().to_path_buf()).unwrap();
            conf.local_conf.borrow_mut().seed = 42;
            let conf: &'static _ = Box::leak(Box::new(conf));
            let mut services = ServiceMap::new(conf);
            modules::MODULES
                .iter()
                .for_each(|&m| services.install_module(m));
            services.write_all().unwrap();
            let has_beam_networks = !conf.local_conf.borrow().beam_networks.is_empty();
            let has_services = services.len() > 0;
            let tmp_dir_path = temp_dir.path().display().to_string();
            let filters = [(tmp_dir_path.as_str(), "[TMP_DIR]")];
            insta::glob!(temp_dir.path(), "**/*", |path| {
                if path.is_dir() || path.extension() == Some("pem".as_ref()) {
                    return;
                }
                let file = std::fs::read_to_string(path).unwrap();
                insta::allow_duplicates! {
                    insta::with_settings!({
                        filters => filters,
                        input_file => &conf_path,
                        snapshot_path => format!("../tests/snapshots/{}", conf_path.file_stem().unwrap().display()),
                        info => &path.strip_prefix(temp_dir.path()).unwrap(),
                    }, {
                        match path.file_name().and_then(|s| s.to_str()?.rsplit_once('.')) {
                            Some((_, "yml")) => insta::assert_snapshot!(file),
                            Some(("config", "toml")) => return,
                            Some(("config.local", "toml")) => insta::assert_toml_snapshot!(toml::from_str::<toml::Table>(&file).unwrap()),
                            _ => insta::assert_snapshot!(file),
                        }
                    });
                };
            });
            if !has_services {
                return;
            }
            if has_beam_networks {
                // Fake enroll
                let priv_key = rcgen::generate_simple_self_signed(vec![conf.site_id.clone()])
                    .unwrap()
                    .signing_key
                    .serialize_pem();
                fs::write(
                    temp_dir
                        .path()
                        .join("pki")
                        .join(format!("{}.priv.pem", conf.site_id)),
                    priv_key,
                )
                .unwrap();
            }
            let out = crate::compose_command(temp_dir.path(), &["config".into()])
                .unwrap()
                .stdout_capture()
                .stderr_capture()
                .unchecked()
                .run()
                .unwrap();
            assert!(
                out.status.success(),
                "Generated invalid compose files\n stderr: {}\n stdout: {}",
                String::from_utf8_lossy(&out.stderr),
                String::from_utf8_lossy(&out.stdout)
            );
        });
    }
}
