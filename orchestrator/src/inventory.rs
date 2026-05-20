use anyhow::{Context, Result};
use log::{info, warn};
use serde::Serialize;

use crate::util::bootc_wrapper::{Bootc, BootcStatus};
use crate::util::executer::Executer;

// Full device inventory serializable to JSON.
#[derive(Debug, Serialize)]
pub struct Inventory {
    pub system: SystemInfo,
    pub deployments: CollectionResult<Vec<RpmOstreeDeployment>>,
    pub bootc_status: CollectionResult<BootcStatus>,
    pub applications: CollectionResult<Vec<ApplicationInfo>>,
}

// Contains either the successfully collected data or an error message if data collection (eg rpm-ostree, podman) failed.
// Serializes to JSON in the format
// { "status": "ok", "data": ... } in the success case and
// { "status": "unavailable", "data": { "reason": "error message" } } in the failure case.
#[derive(Debug, Serialize)]
#[serde(tag = "status", content = "data")]
pub enum CollectionResult<T> {
    #[serde(rename = "ok")]
    Ok(T),
    #[serde(rename = "unavailable")]
    Unavailable { reason: String },
}

// Host system information
#[derive(Debug, Serialize)]
pub struct SystemInfo {
    pub hostname: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
}

// Relevant information about rpm-ostree deployments
// Collected via 'rpm-ostree status --json'.
// If needed, more fields can be added in the future.
#[derive(Debug, Serialize)]
pub struct RpmOstreeDeployment {
    pub checksum: String,
    pub version: String,
    pub is_booted: bool,
    pub is_staged: bool,
    pub origin: String,
}

// Relevant information about applications
// Only basic for now, will be collected via 'podman ps' in the future, fields can be adapted as needed.
#[derive(Debug, Serialize)]
pub struct ApplicationInfo {
    pub app_name: String,
    pub app_version: String,
}

// Collects the inventory and saves it to the given path in JSON format.
// System info is required and if it fails, the entire function returns an error.
// If collection of deployments or applications fails, the error is logged but the function still returns successfully
// with the relevant field in the inventory marked as unavailable with the error message as reason.
// Arguments:
// - inventory_path: the file path where the collected inventory should be saved as JSON (should come from config). The function will create parent directories
//   if they do not exist, and will write atomically to avoid leaving a corrupted inventory file in case of interruption during writing.
// Returns:
// - Ok(()) if the inventory was collected and saved successfully (even if some fields are unavailable due to collection errors)
// - Err(anyhow::Error) if there was an error during system info collection or saving the inventory,
//   with a context message indicating the failure reason.
pub async fn collect_and_save_inventory(
    bootc: &Bootc,
    exec: &dyn Executer,
    inventory_path: &std::path::Path,
) -> Result<()> {
    let inventory = collect_inventory(bootc, exec).await?;

    let json = serde_json::to_string_pretty(&inventory)
        .with_context(|| "Failed to serialize inventory to JSON")?;

    if let Some(parent) = inventory_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create directories for inventory path: {}",
                inventory_path.display()
            )
        })?;
    }

    // Atomic write: write to a temporary file first and then rename it to the final path,
    // to avoid leaving a corrupted inventory file if the process is interrupted during writing.
    let tmp_path = inventory_path.with_extension("tmp");
    std::fs::write(&tmp_path, &json).with_context(|| {
        format!(
            "Failed to write inventory to temporary path: {}",
            tmp_path.display()
        )
    })?;

    // std::fs::rename is atomic
    std::fs::rename(&tmp_path, inventory_path).with_context(|| {
        format!(
            "Failed to move temporary inventory file to final path: {}",
            inventory_path.display()
        )
    })?;

    info!(
        "Successfully collected and saved inventory to {}",
        inventory_path.display()
    );
    Ok(())
}

pub async fn collect_inventory(bootc: &Bootc, exec: &dyn Executer) -> Result<Inventory> {
    Ok(Inventory {
        // System info collection is required, if it fails, we return an error for the entire inventory collection process
        system: collect_system_info()?,
        deployments: match collect_deployments(exec).await {
            Ok(d) => CollectionResult::Ok(d),
            Err(e) => {
                warn!("Could not collect rpm deployments: {}", e);
                CollectionResult::Unavailable {
                    reason: e.to_string(),
                }
            }
        },
        bootc_status: match bootc.status().await {
            Ok(s) => CollectionResult::Ok(s),
            Err(e) => {
                warn!("Could not collect bootc status: {}", e);
                CollectionResult::Unavailable {
                    reason: e.to_string(),
                }
            }
        },
        applications: match collect_applications() {
            Ok(a) => CollectionResult::Ok(a),
            Err(e) => {
                warn!("Could not collect applications: {}", e);
                CollectionResult::Unavailable {
                    reason: e.to_string(),
                }
            }
        },
    })
}

pub async fn healthcheck_inventory(bootc: &Bootc, exec: &dyn Executer) -> Result<()> {
    let _inventory = collect_inventory(bootc, exec).await?;
    Ok(())
}

// --System info collection-------------

fn collect_system_info() -> Result<SystemInfo> {
    Ok(SystemInfo {
        hostname: read_hostname()?,
        os_name: read_os_release_field("NAME")?,
        os_version: read_os_release_field("VERSION_ID")?,
        kernel_version: read_kernel_version()?,
    })
}

fn read_hostname() -> Result<String> {
    std::fs::read_to_string("/etc/hostname")
        .with_context(|| "Failed to read hostname")
        .map(|s| s.trim().to_string())
}

// Reads a specific field from /etc/os-release, which is formatted as KEY="value" per line.
fn read_os_release_field(field: &str) -> Result<String> {
    let os_release_content = std::fs::read_to_string("/etc/os-release")
        .with_context(|| "Failed to read /etc/os-release")?;

    let prefix = format!("{}=", field);
    for line in os_release_content.lines() {
        if let Some(value) = line.strip_prefix(&prefix) {
            return Ok(value.trim_matches('"').to_string());
        }
    }

    anyhow::bail!("Field {} not found in /etc/os-release", field);
}

fn read_kernel_version() -> Result<String> {
    // Format: "Linux version 6.x.y-... { ... }"
    // We want to extract the 6.x.y part, which is the third whitespace-separated token.
    std::fs::read_to_string("/proc/version")
        .with_context(|| "Failed to read kernel version")
        .map(|s| s.split_whitespace().nth(2).unwrap_or("unknown").to_string())
}

// --Rpm-ostree deployments collection-------------

// Collects information about rpm-ostree deployments by running 'rpm-ostree status --json' and parsing the output
// into a vector of RpmOstreeDeployment structs.
async fn collect_deployments(exec: &dyn Executer) -> Result<Vec<RpmOstreeDeployment>> {
    let res = exec
        .execute(
            "rpm-ostree".to_string(),
            vec!["status".to_string(), "--json".to_string()],
        )
        .await?;

    if res.exit_code != Some(0) {
        anyhow::bail!(
            "rpm-ostree command failed with exit code: {:?}",
            res.exit_code
        );
    }

    let json: serde_json::Value = serde_json::from_str(&res.stdout)
        .with_context(|| "Failed to parse rpm-ostree output as JSON")?;

    let deployments = json["deployments"]
        .as_array()
        .with_context(|| "rpm-ostree output JSON does not contain deployments array")?;

    let result = deployments
        .iter()
        .map(|d| RpmOstreeDeployment {
            checksum: d["checksum"].as_str().unwrap_or("").to_string(),
            version: d["version"].as_str().unwrap_or("").to_string(),
            is_booted: d["booted"].as_bool().unwrap_or(false),
            is_staged: d["staged"].as_bool().unwrap_or(false),
            origin: d["origin"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    Ok(result)
}

// --Applications collection-------------

// Collects information about applications. Placeholder for now, in the future this will run e.g. 'podman ps' and parse the output.
fn collect_applications() -> Result<Vec<ApplicationInfo>> {
    Ok(vec![
        ApplicationInfo {
            app_name: "dummy_app_1".to_string(),
            app_version: "v1.2.3".to_string(),
        },
        ApplicationInfo {
            app_name: "dummy_app_2".to_string(),
            app_version: "v4.5.6".to_string(),
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ok_result_serializes_correctly() {
        let result: CollectionResult<Vec<ApplicationInfo>> =
            CollectionResult::Ok(vec![ApplicationInfo {
                app_name: "mock-app".to_string(),
                app_version: "0.1.0".to_string(),
            }]);

        let json = serde_json::to_string(&result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["data"][0]["app_name"], "mock-app");
    }

    #[test]
    fn test_unavailable_result_serializes_correctly() {
        let result: CollectionResult<Vec<RpmOstreeDeployment>> = CollectionResult::Unavailable {
            reason: "rpm-ostree not found".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["status"], "unavailable");
        assert_eq!(parsed["data"]["reason"], "rpm-ostree not found");
    }

    #[test]
    fn test_full_inventory_with_unavailable_deployments() {
        let inventory = Inventory {
            system: SystemInfo {
                hostname: "test-host".to_string(),
                os_name: "Fedora Linux".to_string(),
                os_version: "41".to_string(),
                kernel_version: "6.11.0-300.fc41.x86_64".to_string(),
            },
            deployments: CollectionResult::Unavailable {
                reason: "rpm-ostree not found".to_string(),
            },
            bootc_status: CollectionResult::Unavailable {
                reason: "bootc not found".to_string(),
            },
            applications: CollectionResult::Ok(vec![ApplicationInfo {
                app_name: "mock-app".to_string(),
                app_version: "0.1.0".to_string(),
            }]),
        };

        let json = serde_json::to_string_pretty(&inventory).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["system"]["hostname"], "test-host");
        assert_eq!(parsed["deployments"]["status"], "unavailable");
        assert_eq!(parsed["bootc_status"]["status"], "unavailable");
        assert_eq!(parsed["applications"]["status"], "ok");
    }

    #[test]
    fn test_collect_and_save_writes_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("inventory.json");

        // Write a minimal inventory manually to test the save logic
        let inventory = Inventory {
            system: SystemInfo {
                hostname: "test-host".to_string(),
                os_name: "Fedora Linux".to_string(),
                os_version: "41".to_string(),
                kernel_version: "6.11.0".to_string(),
            },
            deployments: CollectionResult::Unavailable {
                reason: "not an ostree system".to_string(),
            },
            bootc_status: CollectionResult::Unavailable {
                reason: "bootc not found".to_string(),
            },
            applications: CollectionResult::Ok(vec![]),
        };

        let json = serde_json::to_string_pretty(&inventory).unwrap();
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, &json).unwrap();
        std::fs::rename(&tmp, &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["system"]["hostname"], "test-host");
        assert_eq!(parsed["deployments"]["status"], "unavailable");
        assert_eq!(parsed["bootc_status"]["status"], "unavailable");
        assert_eq!(parsed["applications"]["status"], "ok");
    }

    #[test]
    fn test_parse_real_rpm_ostree_output() {
        let raw = r#"{
            "deployments" : [
                {
                    "unlocked" : "none",
                    "requested-local-packages" : [],
                    "base-removals" : [],
                    "gpg-enabled" : true,
                    "pinned" : false,
                    "osname" : "fedora-iot",
                    "origin" : "fedora-iot:fedora/stable/x86_64/iot",
                    "regenerate-initramfs" : false,
                    "checksum" : "029b843f50ab1dd56ecc4d3eabb94f1aace5d958794ae4c2c72a915ee1b10443",
                    "id" : "fedora-iot-029b843f50ab1dd56ecc4d3eabb94f1aace5d958794ae4c2c72a915ee1b10443.0",
                    "version" : "44.20260511.0",
                    "requested-packages" : [],
                    "serial" : 0,
                    "timestamp" : 1778497486,
                    "staged" : false,
                    "booted" : true,
                    "packages" : []
                },
                {
                    "unlocked" : "none",
                    "requested-local-packages" : [],
                    "base-removals" : [],
                    "gpg-enabled" : true,
                    "pinned" : false,
                    "osname" : "fedora-iot",
                    "origin" : "fedora-iot:fedora/stable/x86_64/iot",
                    "regenerate-initramfs" : false,
                    "checksum" : "35a2e036cdcf8f3067effe5a7a7415993481e9beaaca7eed7eabf53381852192",
                    "id" : "fedora-iot-35a2e036cdcf8f3067effe5a7a7415993481e9beaaca7eed7eabf53381852192.0",
                    "version" : "44.20260427.0",
                    "requested-packages" : [],
                    "serial" : 0,
                    "timestamp" : 1777307666,
                    "staged" : false,
                    "booted" : false,
                    "packages" : [],
                    "pending-base-checksum" : "029b843f50ab1dd56ecc4d3eabb94f1aace5d958794ae4c2c72a915ee1b10443"
                }
            ],
            "transaction" : null,
            "cached-update" : null,
            "update-driver" : null
        }"#;

        let json: serde_json::Value = serde_json::from_str(raw).unwrap();
        let deployments = json["deployments"].as_array().unwrap();

        let result: Vec<RpmOstreeDeployment> = deployments
            .iter()
            .map(|d| RpmOstreeDeployment {
                checksum: d["checksum"].as_str().unwrap_or("unknown").to_string(),
                version: d["version"].as_str().unwrap_or("unknown").to_string(),
                is_booted: d["booted"].as_bool().unwrap_or(false),
                is_staged: d["staged"].as_bool().unwrap_or(false),
                origin: d["origin"].as_str().unwrap_or("unknown").to_string(),
            })
            .collect();

        assert_eq!(result.len(), 2);

        // First entry is the booted deployment
        assert_eq!(result[0].is_booted, true);
        assert_eq!(result[0].is_staged, false);
        assert_eq!(result[0].version, "44.20260511.0".to_string());
        assert_eq!(
            result[0].checksum,
            "029b843f50ab1dd56ecc4d3eabb94f1aace5d958794ae4c2c72a915ee1b10443"
        );
        assert_eq!(
            result[0].origin,
            "fedora-iot:fedora/stable/x86_64/iot".to_string()
        );

        // Second entry is the rollback deployment
        assert_eq!(result[1].is_booted, false);
        assert_eq!(result[1].is_staged, false);
        assert_eq!(result[1].version, "44.20260427.0".to_string());
        assert_eq!(
            result[1].checksum,
            "35a2e036cdcf8f3067effe5a7a7415993481e9beaaca7eed7eabf53381852192"
        );
        assert_eq!(
            result[1].origin,
            "fedora-iot:fedora/stable/x86_64/iot".to_string()
        );
    }

    #[test]
    fn test_parse_bootc_output_with_null_image() {
        // Output captured from a basic Fedora IoT VM where image is null
        // since the system was not booted from a container registry
        let raw = r#"{
            "apiVersion": "org.containers.bootc/v1",
            "kind": "BootcHost",
            "metadata": { "name": "host" },
            "spec": { "bootOrder": "default", "image": null },
            "status": {
                "booted": {
                    "cachedUpdate": null,
                    "composefs": null,
                    "downloadOnly": false,
                    "image": null,
                    "incompatible": false,
                    "ostree": {
                        "checksum": "029b843f50ab1dd56ecc4d3eabb94f1aace5d958794ae4c2c72a915ee1b10443",
                        "deploySerial": 0,
                        "stateroot": "fedora-iot"
                    },
                    "pinned": false,
                    "softRebootCapable": true,
                    "store": "ostreeContainer"
                },
                "rollback": {
                    "cachedUpdate": null,
                    "composefs": null,
                    "downloadOnly": false,
                    "image": null,
                    "incompatible": false,
                    "ostree": {
                        "checksum": "35a2e036cdcf8f3067effe5a7a7415993481e9beaaca7eed7eabf53381852192",
                        "deploySerial": 0,
                        "stateroot": "fedora-iot"
                    },
                    "pinned": false,
                    "softRebootCapable": false,
                    "store": "ostreeContainer"
                },
                "rollbackQueued": false,
                "staged": null,
                "type": null,
                "usrOverlay": null
            }
        }"#;

        let json: serde_json::Value = serde_json::from_str(raw).unwrap();
        let status_block = json["status"].to_string();

        let result: BootcStatus = serde_json::from_str(&status_block).unwrap();

        // Booted deployment
        assert_eq!(
            result.booted.checksum,
            "029b843f50ab1dd56ecc4d3eabb94f1aace5d958794ae4c2c72a915ee1b10443"
        );
        assert!(result.booted.image.is_none()); // null on basic VM

        // No staged deployment
        assert!(result.staged.is_none());

        // Rollback deployment
        let rollback = result.rollback.unwrap();
        assert_eq!(
            rollback.checksum,
            "35a2e036cdcf8f3067effe5a7a7415993481e9beaaca7eed7eabf53381852192"
        );
        assert!(rollback.image.is_none());

        assert!(!result.rollback_queued);
    }
}
