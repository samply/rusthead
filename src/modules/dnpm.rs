use serde::Deserialize;

use crate::{
    config::Config,
    modules::{CcpDefault, Module},
    services::{
        BeamConnect, ServiceMap,
        beam_connect::{CentralTarget, LocalTarget},
        dnpm_node::{DnpmNode, DnpmNodeConf},
    },
};

pub struct Dnpm;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DnpmConfig {
    Node(DnpmNodeConf),
    Local {
        target: LocalTarget,
        no_proxy: Option<String>,
    },
}

impl Module for Dnpm {
    fn install(&self, service_map: &mut ServiceMap, global_conf: &'static Config) {
        let Some(conf) = &global_conf.dnpm else {
            return;
        };
        let bc = service_map.install_default::<BeamConnect<CcpDefault>>();
        let mut default_allowed_remotes = vec![];
        for (vhost, beam_connect) in DNPM_SITES {
            bc.add_central_target(CentralTarget {
                virtualhost: vhost.to_string(),
                beam_connect: beam_connect.to_string(),
            });
            default_allowed_remotes.push(beam_connect.to_string());
        }
        match conf {
            DnpmConfig::Node(conf) => {
                bc.add_local_target(LocalTarget::new(
                    format!("{}.dnpm.de", conf.zpm_site.to_lowercase()),
                    "dnpm-backend:9000".to_string(),
                    default_allowed_remotes,
                ));
                service_map.install_with_config::<DnpmNode>((conf.clone(), global_conf));
            }
            DnpmConfig::Local { target, no_proxy } => {
                let mut target = target.clone();
                if target.allowed.is_empty() {
                    target.allowed = default_allowed_remotes;
                }
                bc.add_local_target(target);
                if let Some(no_proxy) = no_proxy {
                    bc.no_proxy.push(no_proxy.clone());
                }
            }
        }
    }
}

const DNPM_SITES: &[(&str, &str)] = &[
    (
        "ukfr.dnpm.de",
        "dnpm-connect.dnpm-bridge.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "ukhd.dnpm.de",
        "dnpm-connect.dnpm-bridge.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "ukt.dnpm.de",
        "dnpm-connect.dnpm-bridge.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "uku.dnpm.de",
        "dnpm-connect.dnpm-bridge.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "um.dnpm.de",
        "dnpm-connect.dnpm-bridge.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "ukmr.dnpm.de",
        "dnpm-connect.dnpm-bridge.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "uke.dnpm.de",
        "dnpm-connect.dnpm-bridge.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "uka.dnpm.de",
        "dnpm-connect.dnpm-bridge.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "charite.dnpm.de",
        "dnpm-connect.berlin-test.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "mri.dnpm.de",
        "dnpm-connect.muenchen-tum.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "kum.dnpm.de",
        "dnpm-connect.muenchen-lmu.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "mhh.dnpm.de",
        "dnpm-connect.hannover.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "ukdd.dnpm.de",
        "dnpm-connect.dresden-dnpm.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "ukb.dnpm.de",
        "dnpm-connect.bonn-dnpm.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "ukd.dnpm.de",
        "dnpm-connect.duesseldorf-dnpm.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "ukk.dnpm.de",
        "dnpm-connect.dnpm-bridge.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "ume.dnpm.de",
        "dnpm-connect.essen.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "ukm.dnpm.de",
        "dnpm-connect.muenster-dnpm.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "ukf.dnpm.de",
        "dnpm-connect.frankfurt.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "umg.dnpm.de",
        "dnpm-connect.goettingen.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "ukw.dnpm.de",
        "dnpm-connect.wuerzburg-dnpm.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "uksh.dnpm.de",
        "dnpm-connect.uksh-dnpm.broker.ccp-it.dktk.dkfz.de",
    ),
    (
        "tkt.dnpm.de",
        "dnpm-connect.tobias-develop.broker.ccp-it.dktk.dkfz.de",
    ),
];
