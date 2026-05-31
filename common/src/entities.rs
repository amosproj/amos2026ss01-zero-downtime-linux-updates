pub mod application;
pub use crate::entities::application as Application;
pub use crate::entities::application::CreateApplication as CreateApplication;

pub mod application_assignment;
pub use crate::entities::application_assignment as ApplicationAssignment;
pub use crate::entities::application_assignment::CreateApplicationAssignment as CreateApplicationAssignment;

pub mod application_config;
pub use crate::entities::application_config as ApplicationConfig;
pub use crate::entities::application_config::CreateApplicationConfig as CreateApplicationConfig;

pub mod device;
pub use crate::entities::device as Device;
pub use crate::entities::device::CreateDevice as CreateDevice;

pub mod group;
pub use crate::entities::group as Group;
pub use crate::entities::group::CreateGroup as CreateGroup;

pub mod os_assignment;
pub use crate::entities::os_assignment as OsAssignment;
pub use crate::entities::os_assignment::CreateOsAssignment as CreateOsAssignment;

pub mod os_version;
pub use crate::entities::os_version as OsVersion;
pub use crate::entities::os_version::CreateOsVersion as CreateOsVersion;

pub mod ping;
pub use crate::entities::ping as Ping;

pub mod reported_application_assignment;
pub use crate::entities::reported_application_assignment as ReportedApplicationAssignment;
pub use crate::entities::reported_application_assignment::CreateReportedApplicationAssignment as CreateReportedApplicationAssignment;

pub mod reported_os_assignment;
pub use crate::entities::reported_os_assignment as ReportedOsAssignment;
pub use crate::entities::reported_os_assignment::CreateReportedOsAssignment as CreateReportedOsAssignment;

pub mod tenant;
pub use crate::entities::tenant as Tenant;
pub use crate::entities::tenant::CreateTenant as CreateTenant;
