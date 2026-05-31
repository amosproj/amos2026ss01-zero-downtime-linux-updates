pub mod application;
pub use self::application as Application;

pub mod application_assignment;
pub use self::application_assignment as ApplicationAssignment;

pub mod application_config;
pub use self::application_config as ApplicationConfig;

pub mod device;
pub use self::device as Device;

pub mod group;
pub use self::group as Group;

pub mod os_assignment;
pub use self::os_assignment as OsAssignment;

pub mod os_version;
pub use self::os_version as OsVersion;

pub mod ping;
pub use self::ping as Ping;

pub mod reported_application_assignment;
pub use self::reported_application_assignment as ReportedApplicationAssignment;

pub mod reported_os_assignment;
pub use self::reported_os_assignment as ReportedOsAssignment;

pub mod tenant;
pub use self::tenant as Tenant;
