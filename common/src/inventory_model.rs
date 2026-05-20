// Shared inventory data model used as the JSON contract between the cloud
// (api-mock-server today) and the orchestrator's update-check flow.
//
// These types intentionally mirror the JSON shape that the orchestrator's
// `Inventory` serializes today (see orchestrator/src/inventory.rs and
// orchestrator/src/util/bootc_wrapper.rs) so the same payload describes both
// what a device currently has and what the cloud wants it to have.
//
// They are NOT identical Rust types to the orchestrator's because the
// orchestrator's bootc types carry a wire-format adapter for the bootc CLI
// output, which is irrelevant on the cloud side.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemInfo {
    pub hostname: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RpmOstreeDeployment {
    pub checksum: String,
    pub version: String,
    pub is_booted: bool,
    pub is_staged: bool,
    pub origin: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApplicationInfo {
    pub app_name: String,
    pub app_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BootcImageInfo {
    pub image_ref: String,
    pub transport: String,
    pub image_digest: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BootcDeploymentInfo {
    pub checksum: String,
    pub image: Option<BootcImageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BootcStatus {
    pub booted: BootcDeploymentInfo,
    pub staged: Option<BootcDeploymentInfo>,
    pub rollback: Option<BootcDeploymentInfo>,
    pub rollback_queued: bool,
}

// Cloud-side authoritative target state. No CollectionResult wrappers — the
// cloud either has a requirement or it doesn't.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemRequirements {
    pub system: SystemInfo,
    pub bootc_status: BootcStatus,
    pub applications: Vec<ApplicationInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_requirements() -> SystemRequirements {
        SystemRequirements {
            system: SystemInfo {
                hostname: "target-host".into(),
                os_name: "Fedora Linux".into(),
                os_version: "41".into(),
                kernel_version: "6.11.0".into(),
            },
            bootc_status: BootcStatus {
                booted: BootcDeploymentInfo {
                    checksum: "abc123".into(),
                    image: Some(BootcImageInfo {
                        image_ref: "ghcr.io/example/os:latest".into(),
                        transport: "registry".into(),
                        image_digest: Some("sha256:deadbeef".into()),
                        version: Some("1.2.3".into()),
                    }),
                },
                staged: None,
                rollback: None,
                rollback_queued: false,
            },
            applications: vec![ApplicationInfo {
                app_name: "data_collector".into(),
                app_version: "v1.0.2".into(),
            }],
        }
    }

    #[test]
    fn system_requirements_round_trip() {
        let original = sample_requirements();
        let json = serde_json::to_string(&original).unwrap();
        let parsed: SystemRequirements = serde_json::from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn bootc_status_uses_camel_case_for_rollback_queued() {
        let req = sample_requirements();
        let json = serde_json::to_value(&req.bootc_status).unwrap();
        assert!(json.get("rollbackQueued").is_some());
        assert!(json.get("rollback_queued").is_none());
    }
}
