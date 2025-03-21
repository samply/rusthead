use std::{fs::{self, Permissions}, path::PathBuf};

use rinja::Template;

use crate::{config::Config, services::BEAM_NETWORKS, utils::filters};

#[derive(Debug, Template)]
#[template(path = "bridgehead")]
pub struct Bridgehead<'c> {
    beam_networks: Vec<String>,
    config_dir: &'c PathBuf,
    pwd: &'c PathBuf,
}

impl<'c> Bridgehead<'c> {
    pub fn new(conf: &'c Config) -> Self {
        Self {
            beam_networks: BEAM_NETWORKS.take(),
            config_dir: &conf.path,
            pwd: &conf.srv_dir,
        }
    }
    
    pub fn write(&self) -> anyhow::Result<()> {
        let path = self.pwd.join("bridgehead");
        fs::write( &path, self.render()?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, Permissions::from_mode(0o755))?;
        }
        Ok(())
    }
}
