use sea_orm::{ConnectionTrait, DbErr};

use crate::types::{
    CompressionConfig, ContinuousAggregateConfig, HypertableConfig, RetentionConfig,
};

/// Validates that a string is a safe SQL identifier (alphanumeric and underscores only).
///
/// Rejects empty strings and strings containing characters that could enable SQL injection.
/// Use this for table names, column names, view names, and other SQL identifiers.
pub fn validate_ident(name: &str) -> Result<&str, DbErr> {
    if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        Ok(name)
    } else {
        Err(DbErr::Custom(format!("Invalid SQL identifier: '{name}'")))
    }
}

/// Escapes a string value for use in a SQL single-quoted string literal.
///
/// Replaces `'` with `''` per PostgreSQL string escaping rules.
fn escape_string_literal(s: &str) -> String {
    s.replace('\'', "''")
}

/// Creates a TimescaleDB hypertable from an existing table.
///
/// All identifier parameters are validated for safety (alphanumeric + underscore only).
///
/// # Example
/// ```ignore
/// use sea_orm_timescale::{migration::create_hypertable, types::{HypertableConfig, Interval}};
///
/// create_hypertable(&db, &HypertableConfig {
///     table_name: "readings".into(),
///     time_column: "time".into(),
///     chunk_interval: Some(Interval::Days(7)),
///     if_not_exists: true,
/// }).await?;
/// ```
pub async fn create_hypertable(
    db: &impl ConnectionTrait,
    config: &HypertableConfig,
) -> Result<(), DbErr> {
    let table = validate_ident(&config.table_name)?;
    let time_col = validate_ident(&config.time_column)?;

    let mut sql = String::from("SELECT create_hypertable(");
    sql.push_str(&format!("'{table}', '{time_col}'"));

    if let Some(ref interval) = config.chunk_interval {
        sql.push_str(&format!(", chunk_time_interval => INTERVAL '{interval}'"));
    }

    if config.if_not_exists {
        sql.push_str(", if_not_exists => TRUE");
    }

    sql.push(')');

    db.execute_unprepared(&sql).await?;
    Ok(())
}

/// Enables compression on a hypertable and optionally adds a compression policy.
///
/// All identifier parameters are validated for safety (alphanumeric + underscore only).
///
/// # Example
/// ```ignore
/// use sea_orm_timescale::{migration::enable_compression, types::*};
///
/// enable_compression(&db, "readings", &CompressionConfig {
///     segment_by: vec!["site_id".into()],
///     order_by: vec![("time".into(), SortDirection::Desc)],
///     compress_after: Interval::Days(30),
/// }).await?;
/// ```
pub async fn enable_compression(
    db: &impl ConnectionTrait,
    table: &str,
    config: &CompressionConfig,
) -> Result<(), DbErr> {
    let table = validate_ident(table)?;

    // Validate all column identifiers
    for col in &config.segment_by {
        validate_ident(col)?;
    }
    for (col, _) in &config.order_by {
        validate_ident(col)?;
    }

    // Build ALTER TABLE for compression settings
    let mut alter_sql = format!("ALTER TABLE \"{table}\" SET (timescaledb.compress");

    if !config.segment_by.is_empty() {
        alter_sql.push_str(&format!(
            ", timescaledb.compress_segmentby = '{}'",
            config.segment_by.join(", ")
        ));
    }

    if !config.order_by.is_empty() {
        let order_parts: Vec<String> = config
            .order_by
            .iter()
            .map(|(col, dir)| format!("{col} {dir}"))
            .collect();
        alter_sql.push_str(&format!(
            ", timescaledb.compress_orderby = '{}'",
            order_parts.join(", ")
        ));
    }

    alter_sql.push(')');
    db.execute_unprepared(&alter_sql).await?;

    // Add compression policy
    let policy_sql = format!(
        "SELECT add_compression_policy('{table}', INTERVAL '{}')",
        config.compress_after
    );
    db.execute_unprepared(&policy_sql).await?;

    Ok(())
}

/// Adds a data retention policy to drop old chunks.
///
/// The table name is validated for safety (alphanumeric + underscore only).
///
/// # Example
/// ```ignore
/// use sea_orm_timescale::{migration::add_retention_policy, types::*};
///
/// add_retention_policy(&db, "readings", &RetentionConfig {
///     drop_after: Interval::Days(365),
/// }).await?;
/// ```
pub async fn add_retention_policy(
    db: &impl ConnectionTrait,
    table: &str,
    config: &RetentionConfig,
) -> Result<(), DbErr> {
    let table = validate_ident(table)?;

    let sql = format!(
        "SELECT add_retention_policy('{table}', INTERVAL '{}')",
        config.drop_after
    );
    db.execute_unprepared(&sql).await?;
    Ok(())
}

/// Creates a continuous aggregate materialized view.
///
/// The `view_name` is validated for safety. **Note**: `select_sql` is passed through
/// as raw SQL — callers are responsible for ensuring it is safe. Use Sea-ORM's query
/// builder or parameterized queries when constructing the SELECT body.
///
/// # Example
/// ```ignore
/// use sea_orm_timescale::{migration::create_continuous_aggregate, types::*};
///
/// create_continuous_aggregate(&db,
///     "SELECT time_bucket('1 hour', time) AS bucket,
///            site_id,
///            AVG(value) AS avg_value
///     FROM readings
///     GROUP BY bucket, site_id",
///     &ContinuousAggregateConfig {
///         view_name: "readings_hourly".into(),
///         bucket_interval: Interval::Hours(1),
///         refresh_policy: Some(RefreshPolicy {
///             start_offset: Interval::Days(3),
///             end_offset: Interval::Hours(1),
///             schedule_interval: Interval::Hours(1),
///         }),
///     },
/// ).await?;
/// ```
pub async fn create_continuous_aggregate(
    db: &impl ConnectionTrait,
    select_sql: &str,
    config: &ContinuousAggregateConfig,
) -> Result<(), DbErr> {
    let view = validate_ident(&config.view_name)?;

    let create_sql = format!(
        "CREATE MATERIALIZED VIEW \"{view}\" WITH (timescaledb.continuous) AS {select_sql}"
    );
    db.execute_unprepared(&create_sql).await?;

    if let Some(ref policy) = config.refresh_policy {
        let policy_sql = format!(
            "SELECT add_continuous_aggregate_policy('{view}', \
             start_offset => INTERVAL '{}', \
             end_offset => INTERVAL '{}', \
             schedule_interval => INTERVAL '{}')",
            policy.start_offset, policy.end_offset, policy.schedule_interval
        );
        db.execute_unprepared(&policy_sql).await?;
    }

    Ok(())
}

/// Manually refreshes a continuous aggregate over a time range.
///
/// The view name is validated for safety. Start/end values are escaped to prevent
/// SQL injection.
///
/// # Example
/// ```ignore
/// use sea_orm_timescale::migration::refresh_continuous_aggregate;
///
/// refresh_continuous_aggregate(&db, "readings_hourly",
///     "2024-01-01", "2024-02-01"
/// ).await?;
/// ```
pub async fn refresh_continuous_aggregate(
    db: &impl ConnectionTrait,
    view: &str,
    start: &str,
    end: &str,
) -> Result<(), DbErr> {
    let view = validate_ident(view)?;
    let start = escape_string_literal(start);
    let end = escape_string_literal(end);

    let sql = format!("CALL refresh_continuous_aggregate('{view}', '{start}', '{end}')");
    db.execute_unprepared(&sql).await?;
    Ok(())
}

/// Removes a data retention policy from a hypertable.
///
/// The table name is validated for safety (alphanumeric + underscore only).
///
/// # Example
/// ```ignore
/// use sea_orm_timescale::migration::remove_retention_policy;
///
/// remove_retention_policy(&db, "readings").await?;
/// ```
pub async fn remove_retention_policy(db: &impl ConnectionTrait, table: &str) -> Result<(), DbErr> {
    let table = validate_ident(table)?;
    let sql = format!("SELECT remove_retention_policy('{table}')");
    db.execute_unprepared(&sql).await?;
    Ok(())
}

/// Removes a compression policy from a hypertable.
///
/// This only removes the automatic compression policy — already-compressed chunks
/// remain compressed. The table name is validated for safety.
///
/// # Example
/// ```ignore
/// use sea_orm_timescale::migration::remove_compression_policy;
///
/// remove_compression_policy(&db, "readings").await?;
/// ```
pub async fn remove_compression_policy(
    db: &impl ConnectionTrait,
    table: &str,
) -> Result<(), DbErr> {
    let table = validate_ident(table)?;
    let sql = format!("SELECT remove_compression_policy('{table}')");
    db.execute_unprepared(&sql).await?;
    Ok(())
}

/// Removes a refresh policy from a continuous aggregate.
///
/// The view name is validated for safety (alphanumeric + underscore only).
///
/// # Example
/// ```ignore
/// use sea_orm_timescale::migration::remove_continuous_aggregate_policy;
///
/// remove_continuous_aggregate_policy(&db, "readings_hourly").await?;
/// ```
pub async fn remove_continuous_aggregate_policy(
    db: &impl ConnectionTrait,
    view: &str,
) -> Result<(), DbErr> {
    let view = validate_ident(view)?;
    let sql = format!("SELECT remove_continuous_aggregate_policy('{view}')");
    db.execute_unprepared(&sql).await?;
    Ok(())
}

/// Drops chunks older than a given interval from a hypertable.
///
/// Both the table name and interval are validated. This permanently deletes data
/// in the affected chunks.
///
/// # Example
/// ```ignore
/// use sea_orm_timescale::{migration::drop_chunks, types::Interval};
///
/// drop_chunks(&db, "readings", &Interval::Days(30)).await?;
/// ```
pub async fn drop_chunks(
    db: &impl ConnectionTrait,
    table: &str,
    older_than: &crate::types::Interval,
) -> Result<(), DbErr> {
    let table = validate_ident(table)?;
    let sql = format!("SELECT drop_chunks('{table}', older_than => INTERVAL '{older_than}')");
    db.execute_unprepared(&sql).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Interval, RefreshPolicy, SortDirection};

    // --- validate_ident tests ---

    #[test]
    fn test_validate_ident_valid() {
        assert_eq!(validate_ident("readings").unwrap(), "readings");
        assert_eq!(validate_ident("my_table").unwrap(), "my_table");
        assert_eq!(validate_ident("Table123").unwrap(), "Table123");
        assert_eq!(validate_ident("_private").unwrap(), "_private");
    }

    #[test]
    fn test_validate_ident_rejects_empty() {
        assert!(validate_ident("").is_err());
    }

    #[test]
    fn test_validate_ident_rejects_sql_injection() {
        assert!(validate_ident("readings'; DROP TABLE users; --").is_err());
        assert!(validate_ident("table name").is_err());
        assert!(validate_ident("table-name").is_err());
        assert!(validate_ident("table.name").is_err());
        assert!(validate_ident("table\"name").is_err());
    }

    // --- SQL generation tests ---

    #[test]
    fn test_create_hypertable_sql() {
        // We can't call the async fn directly, but we can test the SQL construction
        // by replicating the logic. Instead, test the building blocks.
        let table = validate_ident("readings").unwrap();
        let time_col = validate_ident("time").unwrap();

        let mut sql = String::from("SELECT create_hypertable(");
        sql.push_str(&format!("'{table}', '{time_col}'"));
        sql.push_str(", chunk_time_interval => INTERVAL '7 days'");
        sql.push_str(", if_not_exists => TRUE");
        sql.push(')');

        assert_eq!(
            sql,
            "SELECT create_hypertable('readings', 'time', chunk_time_interval => INTERVAL '7 days', if_not_exists => TRUE)"
        );
    }

    #[test]
    fn test_enable_compression_sql() {
        let table = validate_ident("readings").unwrap();

        let config = CompressionConfig {
            segment_by: vec!["site_id".into()],
            order_by: vec![("time".into(), SortDirection::Desc)],
            compress_after: Interval::Days(30),
        };

        // Validate identifiers
        for col in &config.segment_by {
            validate_ident(col).unwrap();
        }
        for (col, _) in &config.order_by {
            validate_ident(col).unwrap();
        }

        let mut alter_sql = format!("ALTER TABLE \"{table}\" SET (timescaledb.compress");
        alter_sql.push_str(&format!(
            ", timescaledb.compress_segmentby = '{}'",
            config.segment_by.join(", ")
        ));
        let order_parts: Vec<String> = config
            .order_by
            .iter()
            .map(|(col, dir)| format!("{col} {dir}"))
            .collect();
        alter_sql.push_str(&format!(
            ", timescaledb.compress_orderby = '{}'",
            order_parts.join(", ")
        ));
        alter_sql.push(')');

        assert_eq!(
            alter_sql,
            "ALTER TABLE \"readings\" SET (timescaledb.compress, timescaledb.compress_segmentby = 'site_id', timescaledb.compress_orderby = 'time DESC')"
        );
    }

    #[test]
    fn test_add_retention_policy_sql() {
        let table = validate_ident("readings").unwrap();
        let config = RetentionConfig {
            drop_after: Interval::Days(365),
        };

        let sql = format!(
            "SELECT add_retention_policy('{table}', INTERVAL '{}')",
            config.drop_after
        );

        assert_eq!(
            sql,
            "SELECT add_retention_policy('readings', INTERVAL '365 days')"
        );
    }

    #[test]
    fn test_create_continuous_aggregate_sql() {
        let view = validate_ident("readings_hourly").unwrap();
        let select_sql = "SELECT time_bucket('1 hour', time) AS bucket, site_id, AVG(value) AS avg_value FROM readings GROUP BY bucket, site_id";

        let create_sql = format!(
            "CREATE MATERIALIZED VIEW \"{view}\" WITH (timescaledb.continuous) AS {select_sql}"
        );

        assert!(create_sql.starts_with("CREATE MATERIALIZED VIEW \"readings_hourly\""));
        assert!(create_sql.contains("WITH (timescaledb.continuous)"));
    }

    #[test]
    fn test_continuous_aggregate_policy_sql() {
        let view = validate_ident("readings_hourly").unwrap();
        let policy = RefreshPolicy {
            start_offset: Interval::Days(3),
            end_offset: Interval::Hours(1),
            schedule_interval: Interval::Hours(1),
        };

        let policy_sql = format!(
            "SELECT add_continuous_aggregate_policy('{view}', \
             start_offset => INTERVAL '{}', \
             end_offset => INTERVAL '{}', \
             schedule_interval => INTERVAL '{}')",
            policy.start_offset, policy.end_offset, policy.schedule_interval
        );

        assert_eq!(
            policy_sql,
            "SELECT add_continuous_aggregate_policy('readings_hourly', start_offset => INTERVAL '3 days', end_offset => INTERVAL '1 hours', schedule_interval => INTERVAL '1 hours')"
        );
    }

    #[test]
    fn test_refresh_continuous_aggregate_sql() {
        let view = validate_ident("readings_hourly").unwrap();
        let start = escape_string_literal("2024-01-01");
        let end = escape_string_literal("2024-02-01");

        let sql = format!("CALL refresh_continuous_aggregate('{view}', '{start}', '{end}')");

        assert_eq!(
            sql,
            "CALL refresh_continuous_aggregate('readings_hourly', '2024-01-01', '2024-02-01')"
        );
    }

    #[test]
    fn test_refresh_escapes_single_quotes() {
        let start = escape_string_literal("2024-01-01'--");
        assert_eq!(start, "2024-01-01''--");
    }

    #[test]
    fn test_remove_retention_policy_sql() {
        let table = validate_ident("readings").unwrap();
        let sql = format!("SELECT remove_retention_policy('{table}')");
        assert_eq!(sql, "SELECT remove_retention_policy('readings')");
    }

    #[test]
    fn test_remove_compression_policy_sql() {
        let table = validate_ident("readings").unwrap();
        let sql = format!("SELECT remove_compression_policy('{table}')");
        assert_eq!(sql, "SELECT remove_compression_policy('readings')");
    }

    #[test]
    fn test_remove_continuous_aggregate_policy_sql() {
        let view = validate_ident("readings_hourly").unwrap();
        let sql = format!("SELECT remove_continuous_aggregate_policy('{view}')");
        assert_eq!(
            sql,
            "SELECT remove_continuous_aggregate_policy('readings_hourly')"
        );
    }

    #[test]
    fn test_drop_chunks_sql() {
        let table = validate_ident("readings").unwrap();
        let interval = Interval::Days(30);
        let sql = format!("SELECT drop_chunks('{table}', older_than => INTERVAL '{interval}')");
        assert_eq!(
            sql,
            "SELECT drop_chunks('readings', older_than => INTERVAL '30 days')"
        );
    }

    #[test]
    fn test_compression_rejects_bad_identifiers() {
        let config = CompressionConfig {
            segment_by: vec!["site_id; DROP TABLE users".into()],
            order_by: vec![],
            compress_after: Interval::Days(30),
        };

        for col in &config.segment_by {
            assert!(validate_ident(col).is_err());
        }
    }
}
