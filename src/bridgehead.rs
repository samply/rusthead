use std::{
    collections::HashSet,
    fs::{self, Permissions},
};

use askama::Template;

use crate::{config::Config, services::BEAM_NETWORKS, utils::filters};

#[derive(Debug, Template)]
#[template(path = "bridgehead")]
pub struct Bridgehead {
    beam_networks: HashSet<String>,
    conf: &'static Config,
}

impl Bridgehead {
    pub fn new(conf: &'static Config) -> Self {
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
