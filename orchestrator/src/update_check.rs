// Compares the device's local inventory against the cloud-side target state
// (SystemRequirements) and produces an UpdateDecision. The HTTP fetch is owned
// by `UpdateChecker`; the pure comparison lives in `compare()` so it can be
// unit-tested without I/O. The caller supplies the local `Inventory`, so the
// checker does not duplicate collection work the surrounding loop already did.

use anyhow::Result;
use log::{debug, warn};
use reqwest::Client as HttpClient;

use amos_common::download_manager::{build_http_client, get_system_requirements};

use crate::inventory::{CollectionResult, Inventory};

// Update reasons are split so the caller can act on OS drift (rpm-ostree
// upgrade + reboot) independently from application drift (which today is
// handled elsewhere and must not trigger a reboot).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateDecision {
    UpToDate,
    UpdateRequired {
        os_reasons: Vec<String>,
        app_reasons: Vec<String>,
    },
}

pub struct UpdateChecker {
    http: HttpClient,
    server_url: String,
}

impl UpdateChecker {
    pub fn new(server_url: String, https_proxy: Option<String>) -> Result<Self> {
        let http = build_http_client(https_proxy.as_deref())?;
        Ok(Self { http, server_url })
    }

    // Fetches the cloud-side requirements and compares them to the supplied
    // local inventory. The inventory is passed in so the caller can reuse a
    // single collection per poll cycle.
    pub async fn check(&self, current: &Inventory) -> Result<UpdateDecision> {
        let target = get_system_requirements(&self.http, &self.server_url).await?;
        Ok(compare(current, &target))
    }
}

// Pure comparison between local inventory and cloud target.
// CollectionResult::Unavailable entries are logged and skipped — an unknown
// local section doesn't fabricate an update reason.
pub fn compare(
    current: &Inventory,
    target: &amos_common::inventory_model::SystemRequirements,
) -> UpdateDecision {
    let mut os_reasons = Vec::new();
    let mut app_reasons = Vec::new();

    if current.system.os_version != target.system.os_version {
        os_reasons.push(format!(
            "OS version drift: current `{}` -> target `{}`",
            current.system.os_version, target.system.os_version
        ));
    }

    match &current.bootc_status {
        CollectionResult::Ok(status) => {
            if status.booted.checksum != target.bootc_status.booted.checksum {
                os_reasons.push(format!(
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
                os_reasons.push(format!(
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
                    Some(curr) => app_reasons.push(format!(
                        "Application `{}` version drift: current `{}` -> target `{}`",
                        target_app.app_name, curr.app_version, target_app.app_version
                    )),
                    None => app_reasons.push(format!(
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
                    app_reasons.push(format!(
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

    if os_reasons.is_empty() && app_reasons.is_empty() {
        UpdateDecision::UpToDate
    } else {
        debug!(
            "Update decision: os_reasons={:?}, app_reasons={:?}",
            os_reasons, app_reasons
        );
        UpdateDecision::UpdateRequired {
            os_reasons,
            app_reasons,
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
    fn bootc_checksum_drift_is_os_only() {
        let current = inventory(
            "41",
            CollectionResult::Ok(local_bootc_with_checksum("abc", None)),
            CollectionResult::Ok(vec![]),
        );
        let target = requirements("41", target_bootc_with_checksum("def", None), vec![]);
        match compare(&current, &target) {
            UpdateDecision::UpdateRequired {
                os_reasons,
                app_reasons,
            } => {
                assert!(os_reasons.iter().any(|r| r.contains("checksum drift")));
                assert!(app_reasons.is_empty());
            }
            other => panic!("expected UpdateRequired, got {:?}", other),
        }
    }

    #[test]
    fn os_version_drift_is_os_only() {
        let current = inventory(
            "40",
            CollectionResult::Ok(local_bootc_with_checksum("abc", None)),
            CollectionResult::Ok(vec![]),
        );
        let target = requirements("41", target_bootc_with_checksum("abc", None), vec![]);
        match compare(&current, &target) {
            UpdateDecision::UpdateRequired {
                os_reasons,
                app_reasons,
            } => {
                assert!(os_reasons.iter().any(|r| r.contains("OS version drift")));
                assert!(app_reasons.is_empty());
            }
            other => panic!("expected UpdateRequired, got {:?}", other),
        }
    }

    #[test]
    fn app_version_drift_is_app_only() {
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
            UpdateDecision::UpdateRequired {
                os_reasons,
                app_reasons,
            } => {
                assert!(os_reasons.is_empty());
                assert!(app_reasons.iter().any(|r| r.contains("data_collector")));
            }
            other => panic!("expected UpdateRequired, got {:?}", other),
        }
    }

    #[test]
    fn missing_target_app_is_app_only() {
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
            UpdateDecision::UpdateRequired {
                os_reasons,
                app_reasons,
            } => {
                assert!(os_reasons.is_empty());
                assert!(app_reasons.iter().any(|r| r.contains("missing")));
            }
            other => panic!("expected UpdateRequired, got {:?}", other),
        }
    }

    #[test]
    fn extra_local_app_is_app_only() {
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
            UpdateDecision::UpdateRequired {
                os_reasons,
                app_reasons,
            } => {
                assert!(os_reasons.is_empty());
                assert!(app_reasons.iter().any(|r| r.contains("should be removed")));
            }
            other => panic!("expected UpdateRequired, got {:?}", other),
        }
    }

    #[test]
    fn os_and_app_drift_split_into_two_buckets() {
        let current = inventory(
            "40",
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
            UpdateDecision::UpdateRequired {
                os_reasons,
                app_reasons,
            } => {
                assert!(os_reasons.iter().any(|r| r.contains("OS version drift")));
                assert!(app_reasons.iter().any(|r| r.contains("data_collector")));
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
}
