// Compares the device's local inventory against the cloud-side target state
// from the entity-typed API (OsVersion, ApplicationConfig) and produces an
// UpdateDecision. The HTTP fetch is wired through `UpdateChecker`; the pure
// comparison lives in `compare_os` / `compare_apps` so it can be unit-tested
// without I/O.

use std::sync::Arc;

use amos_common::entities::{ApplicationConfig, OsVersion};
use anyhow::{Context, Result};
use async_trait::async_trait;
use tracing::debug;

use crate::api_client::ApiClient;
use crate::inventory::{ApplicationInfo, CollectionResult};
use crate::util::bootc_wrapper::BootcStatus;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateDecision<T> {
    UpToDate { target: T },
    UpdateRequired { reasons: Vec<String>, target: T },
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait CheckForUpdate: Send + Sync {
    async fn check_os(
        &self,
        bootc_status: &BootcStatus,
    ) -> Result<UpdateDecision<OsVersion::Model>>;

    /// Returns `Ok(None)` when the local app inventory is unavailable and a
    /// meaningful comparison can't be made (e.g., podman not present).
    async fn check_apps(
        &self,
        current: &CollectionResult<Vec<ApplicationInfo>>,
    ) -> Result<Option<UpdateDecision<Vec<ApplicationConfig::Model>>>>;
}

pub struct UpdateChecker {
    download_manager: Arc<ApiClient>,
}

impl UpdateChecker {
    pub fn new(download_manager: Arc<ApiClient>) -> Self {
        Self { download_manager }
    }
}

#[async_trait]
impl CheckForUpdate for UpdateChecker {
    async fn check_os(
        &self,
        bootc_status: &BootcStatus,
    ) -> Result<UpdateDecision<OsVersion::Model>> {
        let target = self.download_manager.get_target_os_version().await?;
        let booted = bootc_status
            .booted
            .as_ref()
            .context("bootc status reports no booted deployment")?;

        Ok(compare_os(&booted.checksum, target))
    }

    async fn check_apps(
        &self,
        current: &CollectionResult<Vec<ApplicationInfo>>,
    ) -> Result<Option<UpdateDecision<Vec<ApplicationConfig::Model>>>> {
        let target = self
            .download_manager
            .get_target_application_configs()
            .await?;
        Ok(compare_apps(current, target))
    }
}

pub fn compare_os(
    booted_checksum: &str,
    target: OsVersion::Model,
) -> UpdateDecision<OsVersion::Model> {
    if booted_checksum == target.commit_hash {
        return UpdateDecision::UpToDate { target };
    }

    let reasons = vec![format!(
        "OS checksum drift: booted `{}` -> target `{}` ({}#{})",
        booted_checksum, target.commit_hash, target.orchestrator_version, target.id
    )];

    debug!("OS update decision reasons: {:?}", reasons);
    UpdateDecision::UpdateRequired { reasons, target }
}

// Match local apps to target configs by image string. Local `ApplicationInfo`
// (currently a stub in `inventory.rs`) gets keyed as `name:version` so the
// comparison logic is correct for the day local collection emits real image
// references; for now it will report drift for every target config.
pub fn compare_apps(
    current: &CollectionResult<Vec<ApplicationInfo>>,
    target: Vec<ApplicationConfig::Model>,
) -> Option<UpdateDecision<Vec<ApplicationConfig::Model>>> {
    let apps = match current {
        CollectionResult::Ok(apps) => apps,
        CollectionResult::Unavailable { reason } => {
            debug!(
                "Skipping app comparison, local inventory unavailable: {}",
                reason
            );
            return None;
        }
    };

    let current_images: Vec<String> = apps.iter().map(ApplicationInfo::image_key).collect();
    let diff = diff_app_images(&current_images, &target);

    let mut reasons = Vec::new();
    for cfg in &diff.to_create {
        reasons.push(format!(
            "Application image `{}` missing (config #{})",
            cfg.image, cfg.id
        ));
    }
    for img in &diff.to_remove {
        reasons.push(format!("Application image `{}` should be removed", img));
    }

    if reasons.is_empty() {
        return Some(UpdateDecision::UpToDate { target });
    }

    debug!("App update decision reasons: {:?}", reasons);
    Some(UpdateDecision::UpdateRequired { reasons, target })
}

// The image-level changes needed to reconcile the running containers against
// the target configs, keyed by image string. Shared by `compare_apps` (to
// build human-readable reasons) and the apps loop's reconciliation so the two
// can't drift apart.
pub(crate) struct AppImageDiff<'a> {
    pub to_create: Vec<&'a ApplicationConfig::Model>,
    pub to_remove: Vec<String>,
}

pub(crate) fn diff_app_images<'a>(
    current_images: &[String],
    target: &'a [ApplicationConfig::Model],
) -> AppImageDiff<'a> {
    let to_create = target
        .iter()
        .filter(|cfg| !current_images.iter().any(|img| img == &cfg.image))
        .collect();
    let to_remove = current_images
        .iter()
        .filter(|img| !target.iter().any(|cfg| &cfg.image == *img))
        .cloned()
        .collect();
    AppImageDiff {
        to_create,
        to_remove,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os_target(id: i32, commit: &str) -> OsVersion::Model {
        OsVersion::Model {
            id,
            commit_hash: commit.into(),
            orchestrator_version: "0.1.0".into(),
            description: None,
            deleted_at: None,
            superseded_by: None,
        }
    }

    fn app_cfg(id: i32, image: &str) -> ApplicationConfig::Model {
        ApplicationConfig::Model {
            id,
            device_id: None,
            group_id: Some(1),
            application_id: 1,
            image: image.into(),
            config: None,
            version: 1,
            deleted_at: None,
            superseded_by: None,
        }
    }

    fn app_info(name: &str, version: &str) -> ApplicationInfo {
        ApplicationInfo {
            app_name: name.into(),
            app_version: version.into(),
        }
    }

    #[test]
    fn compare_os_up_to_date_when_checksum_matches() {
        let target = os_target(1, "abc123");
        match compare_os("abc123", target) {
            UpdateDecision::UpToDate { target } => assert_eq!(target.id, 1),
            _ => panic!("expected UpToDate"),
        }
    }

    #[test]
    fn compare_os_reports_drift_with_reasons() {
        let target = os_target(7, "newhash");
        match compare_os("oldhash", target) {
            UpdateDecision::UpdateRequired { reasons, target } => {
                assert_eq!(target.commit_hash, "newhash");
                assert_eq!(reasons.len(), 1);
                assert!(reasons[0].contains("oldhash"));
                assert!(reasons[0].contains("newhash"));
            }
            _ => panic!("expected UpdateRequired"),
        }
    }

    #[test]
    fn compare_apps_up_to_date_when_images_match() {
        let current = CollectionResult::Ok(vec![app_info("app", "1.0")]);
        let target = vec![app_cfg(1, "app:1.0")];
        match compare_apps(&current, target) {
            Some(UpdateDecision::UpToDate { target }) => assert_eq!(target[0].id, 1),
            _ => panic!("expected UpToDate"),
        }
    }

    #[test]
    fn compare_apps_reports_missing_image() {
        let current = CollectionResult::Ok(vec![]);
        let target = vec![app_cfg(2, "ghcr.io/x/y:1")];
        match compare_apps(&current, target) {
            Some(UpdateDecision::UpdateRequired { reasons, target }) => {
                assert_eq!(reasons.len(), 1);
                assert!(reasons[0].contains("missing"));
                assert_eq!(target[0].image, "ghcr.io/x/y:1");
            }
            _ => panic!("expected UpdateRequired"),
        }
    }

    #[test]
    fn compare_apps_reports_extra_image_for_removal() {
        let current = CollectionResult::Ok(vec![app_info("legacy", "1.0")]);
        let target: Vec<ApplicationConfig::Model> = vec![];
        match compare_apps(&current, target) {
            Some(UpdateDecision::UpdateRequired { reasons, .. }) => {
                assert_eq!(reasons.len(), 1);
                assert!(reasons[0].contains("removed"));
                assert!(reasons[0].contains("legacy:1.0"));
            }
            _ => panic!("expected UpdateRequired"),
        }
    }

    #[test]
    fn compare_apps_returns_none_when_inventory_unavailable() {
        let current = CollectionResult::Unavailable {
            reason: "podman not found".into(),
        };
        let target = vec![app_cfg(3, "app:9")];
        assert!(compare_apps(&current, target).is_none());
    }
}
