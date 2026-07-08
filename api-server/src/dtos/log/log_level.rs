use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "log_level")]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    #[sea_orm(string_value = "trace")]
    Trace,
    #[sea_orm(string_value = "debug")]
    Debug,
    #[sea_orm(string_value = "info")]
    Info,
    #[sea_orm(string_value = "warn")]
    Warn,
    #[sea_orm(string_value = "error")]
    Error,
    #[sea_orm(string_value = "fatal")]
    Fatal,
}

impl From<amos_common::entities::LogLevel> for LogLevel {
    fn from(value: amos_common::entities::LogLevel) -> Self {
        match value {
            amos_common::entities::LogLevel::Trace => LogLevel::Trace,
            amos_common::entities::LogLevel::Debug => LogLevel::Debug,
            amos_common::entities::LogLevel::Info => LogLevel::Info,
            amos_common::entities::LogLevel::Warn => LogLevel::Warn,
            amos_common::entities::LogLevel::Error => LogLevel::Error,
            amos_common::entities::LogLevel::Fatal => LogLevel::Fatal,
        }
    }
}

impl From<LogLevel> for amos_common::entities::LogLevel {
    fn from(value: LogLevel) -> Self {
        match value {
            LogLevel::Trace => amos_common::entities::LogLevel::Trace,
            LogLevel::Debug => amos_common::entities::LogLevel::Debug,
            LogLevel::Info => amos_common::entities::LogLevel::Info,
            LogLevel::Warn => amos_common::entities::LogLevel::Warn,
            LogLevel::Error => amos_common::entities::LogLevel::Error,
            LogLevel::Fatal => amos_common::entities::LogLevel::Fatal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LogLevel;
    use amos_common::entities::LogLevel as ApiLogLevel;

    #[test]
    fn conversions_round_trip_for_all_variants() {
        let pairs = [
            (LogLevel::Trace, ApiLogLevel::Trace),
            (LogLevel::Debug, ApiLogLevel::Debug),
            (LogLevel::Info, ApiLogLevel::Info),
            (LogLevel::Warn, ApiLogLevel::Warn),
            (LogLevel::Error, ApiLogLevel::Error),
            (LogLevel::Fatal, ApiLogLevel::Fatal),
        ];

        for (dto, api) in pairs {
            assert_eq!(ApiLogLevel::from(dto.clone()), api);
            assert_eq!(LogLevel::from(api), dto);
        }
    }
}
