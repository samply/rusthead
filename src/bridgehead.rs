use std::{
    collections::HashSet,
    fs::{self, Permissions},
};

use rinja::Template;

use crate::{config::Config, services::BEAM_NETWORKS, utils::filters};

#[derive(Debug, Template)]
#[template(path = "bridgehead")]
pub struct Bridgehead<'c> {
    beam_networks: HashSet<String>,
    conf: &'c Config,
}

impl<'c> Bridgehead<'c> {
    pub fn new(conf: &'c Config) -> Self {
        Self {
            beam_networks: BEAM_NETWORKS.take(),
            conf,
        }
    }

    pub fn write(&self) -> anyhow::Result<()> {
        let path = self.conf.srv_dir.join("bridgehead");
        fs::write(&path, self.render()?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, Permissions::from_mode(0o755))?;
        }
        Ok(())
    }
}
