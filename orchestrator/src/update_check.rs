// Compares the device's local inventory against the cloud-side target state
// (SystemRequirements) and produces an UpdateDecision. The HTTP fetch and the
// local collection are wired together by `UpdateChecker`; the pure comparison
// lives in `compare()` so it can be unit-tested without I/O.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use log::{debug, warn};
use reqwest::Client as HttpClient;

use amos_common::download_manager::{
    Config as DownloadManagerConfig, build_http_client, get_system_requirements,
};
use amos_common::inventory_model::SystemRequirements;

use crate::inventory::{CollectionResult, Inventory, collect_inventory};
use crate::util::bootc_wrapper::Bootc;
use crate::util::executer::Executer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateDecision {
    UpToDate,
    UpdateRequired {
        reasons: Vec<String>,
        target_os_version: String,
    },
}

pub struct UpdateChecker {
    http: HttpClient,
    dm_config: DownloadManagerConfig,
    bootc: Arc<Bootc>,
    exec: Arc<dyn Executer>,
}

impl UpdateChecker {
    pub fn new(
        server_url: String,
        https_proxy: Option<String>,
        download_dir: PathBuf,
        bootc: Arc<Bootc>,
        exec: Arc<dyn Executer>,
    ) -> Result<Self> {
        let dm_config = DownloadManagerConfig {
            server_url,
            https_proxy,
            download_dir,
        };
        let http = build_http_client(&dm_config)?;
        Ok(Self {
            http,
            dm_config,
            bootc,
            exec,
        })
    }

    pub async fn check(&self) -> Result<UpdateDecision> {
        let target = get_system_requirements(&self.http, &self.dm_config).await?;
        let current = collect_inventory(self.bootc.as_ref(), self.exec.as_ref()).await?;
        Ok(compare(&current, &target))
    }
}

// Pure comparison between local inventory and cloud target.
// CollectionResult::Unavailable entries are logged and skipped — an unknown
// local section doesn't fabricate an update reason.
pub fn compare(current: &Inventory, target: &SystemRequirements) -> UpdateDecision {
    let mut reasons = Vec::new();

    if current.system.os_version != target.system.os_version {
        reasons.push(format!(
            "OS version drift: current `{}` -> target `{}`",
            current.system.os_version, target.system.os_version
        ));
    }

    match &current.bootc_status {
        CollectionResult::Ok(status) => {
            if status.booted.checksum != target.bootc_status.booted.checksum {
                reasons.push(format!(
                    "Bootc booted checksum drift: current `{}` -> target `{}`",
                    status.booted.checksum, target.bootc_status.booted.checksum
                ));
            }
            let current_image_ref = status.booted.image.as_ref().map(|i| i.image_ref.as_str());
            let target_image_ref = target
                .bootc_status
                .booted
                .image
                .as_ref()
                .map(|i| i.image_ref.as_str());
            if current_image_ref != target_image_ref {
                reasons.push(format!(
                    "Bootc booted image_ref drift: current {:?} -> target {:?}",
                    current_image_ref, target_image_ref
                ));
            }
        }
        CollectionResult::Unavailable { reason } => {
            warn!("Skipping bootc comparison, status unavailable: {}", reason);
        }
    }

    match &current.applications {
        CollectionResult::Ok(apps) => {
            for target_app in &target.applications {
                match apps.iter().find(|a| a.app_name == target_app.app_name) {
                    Some(curr) if curr.app_version == target_app.app_version => {}
                    Some(curr) => reasons.push(format!(
                        "Application `{}` version drift: current `{}` -> target `{}`",
                        target_app.app_name, curr.app_version, target_app.app_version
                    )),
                    None => reasons.push(format!(
                        "Application `{}` missing: target version `{}`",
                        target_app.app_name, target_app.app_version
                    )),
                }
            }
            for curr in apps {
                if !target
                    .applications
                    .iter()
                    .any(|t| t.app_name == curr.app_name)
                {
                    reasons.push(format!(
                        "Application `{}` should be removed (current version `{}`)",
                        curr.app_name, curr.app_version
                    ));
                }
            }
        }
        CollectionResult::Unavailable { reason } => {
            warn!(
                "Skipping applications comparison, status unavailable: {}",
                reason
            );
        }
    }

    if reasons.is_empty() {
        UpdateDecision::UpToDate
    } else {
        debug!("Update decision reasons: {:?}", reasons);
        UpdateDecision::UpdateRequired {
            reasons,
            target_os_version: target.system.os_version.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{ApplicationInfo, SystemInfo};
    use crate::util::bootc_wrapper::{
        BootcDeploymentInfo as LocalBootcDeploymentInfo, BootcImageInfo as LocalBootcImageInfo,
        BootcStatus as LocalBootcStatus,
    };
    use amos_common::inventory_model::{
        ApplicationInfo as TargetApp, BootcDeploymentInfo, BootcImageInfo, BootcStatus,
        SystemInfo as TargetSystemInfo, SystemRequirements,
    };

    fn local_bootc_with_checksum(checksum: &str, image_ref: Option<&str>) -> LocalBootcStatus {
        LocalBootcStatus {
            booted: LocalBootcDeploymentInfo {
                checksum: checksum.into(),
                image: image_ref.map(|r| LocalBootcImageInfo {
                    image_ref: r.into(),
                    transport: "registry".into(),
                    image_digest: None,
                    version: None,
                }),
            },
            staged: None,
            rollback: None,
            rollback_queued: false,
        }
    }

    fn target_bootc_with_checksum(checksum: &str, image_ref: Option<&str>) -> BootcStatus {
        BootcStatus {
            booted: BootcDeploymentInfo {
                checksum: checksum.into(),
                image: image_ref.map(|r| BootcImageInfo {
                    image_ref: r.into(),
                    transport: "registry".into(),
                    image_digest: None,
                    version: None,
                }),
            },
            staged: None,
            rollback: None,
            rollback_queued: false,
        }
    }

    fn inventory(
        os_version: &str,
        bootc: CollectionResult<LocalBootcStatus>,
        apps: CollectionResult<Vec<ApplicationInfo>>,
    ) -> Inventory {
        Inventory {
            system: SystemInfo {
                hostname: "host".into(),
                os_name: "Fedora Linux".into(),
                os_version: os_version.into(),
                kernel_version: "6.11.0".into(),
            },
            deployments: CollectionResult::Unavailable {
                reason: "not relevant".into(),
            },
            bootc_status: bootc,
            applications: apps,
        }
    }

    fn requirements(
        os_version: &str,
        bootc: BootcStatus,
        apps: Vec<TargetApp>,
    ) -> SystemRequirements {
        SystemRequirements {
            system: TargetSystemInfo {
                hostname: "host".into(),
                os_name: "Fedora Linux".into(),
                os_version: os_version.into(),
                kernel_version: "6.11.0".into(),
            },
            bootc_status: bootc,
            applications: apps,
        }
    }

    #[test]
    fn matching_state_is_up_to_date() {
        let current = inventory(
            "41",
            CollectionResult::Ok(local_bootc_with_checksum("abc", Some("ghcr.io/example/os"))),
            CollectionResult::Ok(vec![ApplicationInfo {
                app_name: "data_collector".into(),
                app_version: "v1.0.0".into(),
            }]),
        );
        let target = requirements(
            "41",
            target_bootc_with_checksum("abc", Some("ghcr.io/example/os")),
            vec![TargetApp {
                app_name: "data_collector".into(),
                app_version: "v1.0.0".into(),
            }],
        );
        assert_eq!(compare(&current, &target), UpdateDecision::UpToDate);
    }

    #[test]
    fn bootc_checksum_drift_triggers_update() {
        let current = inventory(
            "41",
            CollectionResult::Ok(local_bootc_with_checksum("abc", None)),
            CollectionResult::Ok(vec![]),
        );
        let target = requirements("41", target_bootc_with_checksum("def", None), vec![]);
        match compare(&current, &target) {
            UpdateDecision::UpdateRequired { reasons, .. } => {
                assert!(reasons.iter().any(|r| r.contains("checksum drift")));
            }
            other => panic!("expected UpdateRequired, got {:?}", other),
        }
    }

    #[test]
    fn os_version_drift_triggers_update() {
        let current = inventory(
            "40",
            CollectionResult::Ok(local_bootc_with_checksum("abc", None)),
            CollectionResult::Ok(vec![]),
        );
        let target = requirements("41", target_bootc_with_checksum("abc", None), vec![]);
        match compare(&current, &target) {
            UpdateDecision::UpdateRequired { reasons, .. } => {
                assert!(reasons.iter().any(|r| r.contains("OS version drift")));
            }
            other => panic!("expected UpdateRequired, got {:?}", other),
        }
    }

    #[test]
    fn app_version_drift_triggers_update() {
        let current = inventory(
            "41",
            CollectionResult::Ok(local_bootc_with_checksum("abc", None)),
            CollectionResult::Ok(vec![ApplicationInfo {
                app_name: "data_collector".into(),
                app_version: "v1.0.0".into(),
            }]),
        );
        let target = requirements(
            "41",
            target_bootc_with_checksum("abc", None),
            vec![TargetApp {
                app_name: "data_collector".into(),
                app_version: "v1.0.1".into(),
            }],
        );
        match compare(&current, &target) {
            UpdateDecision::UpdateRequired { reasons, .. } => {
                assert!(reasons.iter().any(|r| r.contains("data_collector")));
            }
            other => panic!("expected UpdateRequired, got {:?}", other),
        }
    }

    #[test]
    fn missing_target_app_triggers_update() {
        let current = inventory(
            "41",
            CollectionResult::Ok(local_bootc_with_checksum("abc", None)),
            CollectionResult::Ok(vec![]),
        );
        let target = requirements(
            "41",
            target_bootc_with_checksum("abc", None),
            vec![TargetApp {
                app_name: "data_collector".into(),
                app_version: "v1.0.0".into(),
            }],
        );
        match compare(&current, &target) {
            UpdateDecision::UpdateRequired { reasons, .. } => {
                assert!(reasons.iter().any(|r| r.contains("missing")));
            }
            other => panic!("expected UpdateRequired, got {:?}", other),
        }
    }

    #[test]
    fn extra_local_app_triggers_removal() {
        let current = inventory(
            "41",
            CollectionResult::Ok(local_bootc_with_checksum("abc", None)),
            CollectionResult::Ok(vec![ApplicationInfo {
                app_name: "stale_app".into(),
                app_version: "v0.0.1".into(),
            }]),
        );
        let target = requirements("41", target_bootc_with_checksum("abc", None), vec![]);
        match compare(&current, &target) {
            UpdateDecision::UpdateRequired { reasons, .. } => {
                assert!(reasons.iter().any(|r| r.contains("should be removed")));
            }
            other => panic!("expected UpdateRequired, got {:?}", other),
        }
    }

    #[test]
    fn unavailable_bootc_is_skipped_not_flagged() {
        let current = inventory(
            "41",
            CollectionResult::Unavailable {
                reason: "bootc not installed".into(),
            },
            CollectionResult::Ok(vec![]),
        );
        let target = requirements("41", target_bootc_with_checksum("abc", None), vec![]);
        assert_eq!(compare(&current, &target), UpdateDecision::UpToDate);
    }

    #[test]
    fn target_os_version_is_propagated_in_update_required() {
        let current = inventory(
            "40",
            CollectionResult::Ok(local_bootc_with_checksum("abc", None)),
            CollectionResult::Ok(vec![]),
        );
        let target = requirements("41", target_bootc_with_checksum("abc", None), vec![]);
        match compare(&current, &target) {
            UpdateDecision::UpdateRequired {
                target_os_version, ..
            } => {
                assert_eq!(target_os_version, "41");
            }
            other => panic!("expected UpdateRequired, got {:?}", other),
        }
    }
}
