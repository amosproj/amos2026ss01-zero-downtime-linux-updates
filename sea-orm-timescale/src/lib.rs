//! # sea-orm-timescale
//!
//! TimescaleDB extension for Sea-ORM providing:
//! - **Query functions**: `time_bucket()`, `time_bucket_gapfill()`, `first()`, `last()`, `locf()`, `histogram()`
//! - **Migration helpers**: Create hypertables, enable compression, set retention policies, create continuous aggregates
//! - **Type-safe intervals**: Parse and represent PostgreSQL intervals
//!
//! ## Quick Start
//!
//! ```ignore
//! use sea_orm_timescale::{functions::time_bucket, types::Interval};
//! use sea_orm::entity::prelude::*;
//!
//! // In a migration:
//! use sea_orm_timescale::migration::create_hypertable;
//! use sea_orm_timescale::types::HypertableConfig;
//!
//! create_hypertable(&db, &HypertableConfig {
//!     table_name: "readings".into(),
//!     time_column: "time".into(),
//!     chunk_interval: Some(Interval::Days(7)),
//!     if_not_exists: true,
//! }).await?;
//!
//! // In a query:
//! let hourly = readings::Entity::find()
//!     .select_only()
//!     .column_as(time_bucket(&Interval::Hours(1), readings::Column::Time), "bucket")
//!     .column_as(Expr::col(readings::Column::Value).avg(), "avg_value")
//!     .group_by(time_bucket(&Interval::Hours(1), readings::Column::Time))
//!     .into_model::<HourlyAvg>()
//!     .all(&db).await?;
//! ```

pub mod functions;
pub mod migration;
pub mod types;
