//! End-to-end tests against a real TimescaleDB instance.
//!
//! These tests are `#[ignore]` by default and require:
//! - A running TimescaleDB instance with the `timescaledb` extension enabled
//! - `DATABASE_URL` environment variable set
//!
//! Run with: `cargo test -- --ignored --test-threads=1`

use sea_orm::sea_query::{Alias, SimpleExpr};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, DbErr, Statement};
use sea_orm_timescale::functions::*;
use sea_orm_timescale::migration::*;
use sea_orm_timescale::types::*;
use uuid::Uuid;

async fn connect_db() -> DatabaseConnection {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for E2E tests");
    let db = Database::connect(&url)
        .await
        .expect("Failed to connect to database");

    // Ensure TimescaleDB extension is enabled
    db.execute_unprepared("CREATE EXTENSION IF NOT EXISTS timescaledb")
        .await
        .expect("Failed to create timescaledb extension");

    db
}

fn unique_name(prefix: &str) -> String {
    let id = Uuid::new_v4().simple().to_string();
    format!("{}_{}", prefix, &id[..8])
}

fn expr_sql(expr: &SimpleExpr) -> &str {
    match expr {
        SimpleExpr::Custom(s) => s.as_str(),
        _ => panic!("Expected SimpleExpr::Custom"),
    }
}

async fn create_test_table(db: &DatabaseConnection, name: &str) {
    let sql = format!(
        r#"CREATE TABLE "{name}" (
            time TIMESTAMPTZ NOT NULL,
            sensor_id TEXT NOT NULL,
            value DOUBLE PRECISION NOT NULL
        )"#
    );
    db.execute_unprepared(&sql)
        .await
        .expect("Failed to create test table");
}

async fn drop_table(db: &DatabaseConnection, name: &str) {
    let _ = db
        .execute_unprepared(&format!(r#"DROP TABLE IF EXISTS "{name}" CASCADE"#))
        .await;
}

async fn drop_view(db: &DatabaseConnection, name: &str) {
    let _ = db
        .execute_unprepared(&format!(
            r#"DROP MATERIALIZED VIEW IF EXISTS "{name}" CASCADE"#
        ))
        .await;
}

/// Creates a hypertable with sample data spanning 3 hours across 2 sensors.
async fn setup_hypertable_with_data(db: &DatabaseConnection) -> String {
    let table = unique_name("ts");
    create_test_table(db, &table).await;

    create_hypertable(
        db,
        &HypertableConfig {
            table_name: table.clone(),
            time_column: "time".into(),
            chunk_interval: Some(Interval::Days(7)),
            if_not_exists: true,
        },
    )
    .await
    .unwrap();

    let sql = format!(
        r#"INSERT INTO "{table}" (time, sensor_id, value) VALUES
            ('2024-01-01 00:10:00+00', 'a', 10.0),
            ('2024-01-01 00:40:00+00', 'a', 20.0),
            ('2024-01-01 01:10:00+00', 'a', 30.0),
            ('2024-01-01 01:40:00+00', 'b', 40.0),
            ('2024-01-01 02:10:00+00', 'b', 50.0)"#
    );
    db.execute_unprepared(&sql).await.unwrap();

    table
}

#[tokio::test]
#[ignore]
async fn test_create_hypertable() {
    let db = connect_db().await;
    let table = unique_name("ht");
    create_test_table(&db, &table).await;

    create_hypertable(
        &db,
        &HypertableConfig {
            table_name: table.clone(),
            time_column: "time".into(),
            chunk_interval: Some(Interval::Days(7)),
            if_not_exists: true,
        },
    )
    .await
    .expect("create_hypertable should succeed");

    // Verify it appears in the TimescaleDB catalog
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "SELECT hypertable_name FROM timescaledb_information.hypertables \
                 WHERE hypertable_name = '{table}'"
            ),
        ))
        .await
        .expect("catalog query failed");
    assert!(row.is_some(), "Table should appear in hypertables catalog");

    drop_table(&db, &table).await;
}

#[tokio::test]
#[ignore]
async fn test_time_bucket_query() {
    let db = connect_db().await;
    let table = setup_hypertable_with_data(&db).await;

    let bucket = time_bucket(&Interval::Hours(1), Alias::new("time"));
    let sql = format!(
        r#"SELECT {bucket} AS bucket, AVG(value) AS avg_val
           FROM "{table}" GROUP BY bucket ORDER BY bucket"#,
        bucket = expr_sql(&bucket)
    );
    let rows = db
        .query_all(Statement::from_string(DbBackend::Postgres, sql))
        .await
        .expect("time_bucket query failed");

    // Data spans hours 00, 01, 02 -> 3 buckets
    assert_eq!(rows.len(), 3, "Expected 3 hourly buckets");

    drop_table(&db, &table).await;
}

#[tokio::test]
#[ignore]
async fn test_first_last_aggregates() {
    let db = connect_db().await;
    let table = setup_hypertable_with_data(&db).await;

    let first_expr = first(Alias::new("value"), Alias::new("time"));
    let last_expr = last(Alias::new("value"), Alias::new("time"));
    let sql = format!(
        r#"SELECT {first} AS first_val, {last} AS last_val FROM "{table}""#,
        first = expr_sql(&first_expr),
        last = expr_sql(&last_expr)
    );

    let row = db
        .query_one(Statement::from_string(DbBackend::Postgres, sql))
        .await
        .expect("first/last query failed")
        .expect("Should return a row");

    let first_val: f64 = row.try_get("", "first_val").unwrap();
    let last_val: f64 = row.try_get("", "last_val").unwrap();

    assert_eq!(first_val, 10.0, "first() should return earliest value");
    assert_eq!(last_val, 50.0, "last() should return latest value");

    drop_table(&db, &table).await;
}

#[tokio::test]
#[ignore]
async fn test_enable_compression() {
    let db = connect_db().await;
    let table = setup_hypertable_with_data(&db).await;

    enable_compression(
        &db,
        &table,
        &CompressionConfig {
            segment_by: vec!["sensor_id".into()],
            order_by: vec![("time".into(), SortDirection::Desc)],
            compress_after: Interval::Days(30),
        },
    )
    .await
    .expect("enable_compression should succeed");

    drop_table(&db, &table).await;
}

#[tokio::test]
#[ignore]
async fn test_add_retention_policy() {
    let db = connect_db().await;
    let table = setup_hypertable_with_data(&db).await;

    add_retention_policy(
        &db,
        &table,
        &RetentionConfig {
            drop_after: Interval::Days(365),
        },
    )
    .await
    .expect("add_retention_policy should succeed");

    drop_table(&db, &table).await;
}

#[tokio::test]
#[ignore]
async fn test_continuous_aggregate() {
    let db = connect_db().await;
    let table = setup_hypertable_with_data(&db).await;
    let view = unique_name("cagg");

    let select_sql = format!(
        r#"SELECT time_bucket('1 hour', time) AS bucket,
                  sensor_id,
                  AVG(value) AS avg_value
           FROM "{table}"
           GROUP BY bucket, sensor_id"#
    );

    create_continuous_aggregate(
        &db,
        &select_sql,
        &ContinuousAggregateConfig {
            view_name: view.clone(),
            bucket_interval: Interval::Hours(1),
            refresh_policy: None,
        },
    )
    .await
    .expect("create_continuous_aggregate should succeed");

    refresh_continuous_aggregate(&db, &view, "2024-01-01", "2024-01-02")
        .await
        .expect("refresh_continuous_aggregate should succeed");

    // Verify data materialized in the view
    let rows = db
        .query_all(Statement::from_string(
            DbBackend::Postgres,
            format!(r#"SELECT * FROM "{view}""#),
        ))
        .await
        .expect("query on continuous aggregate failed");

    assert!(
        !rows.is_empty(),
        "Continuous aggregate should contain data after refresh"
    );

    drop_view(&db, &view).await;
    drop_table(&db, &table).await;
}

#[tokio::test]
#[ignore]
async fn test_interpolate_query() {
    let db = connect_db().await;
    let table = unique_name("ts");
    create_test_table(&db, &table).await;

    create_hypertable(
        &db,
        &HypertableConfig {
            table_name: table.clone(),
            time_column: "time".into(),
            chunk_interval: Some(Interval::Days(7)),
            if_not_exists: true,
        },
    )
    .await
    .unwrap();

    // Insert data with a gap at hour 1
    let sql = format!(
        r#"INSERT INTO "{table}" (time, sensor_id, value) VALUES
            ('2024-01-01 00:00:00+00', 'a', 10.0),
            ('2024-01-01 02:00:00+00', 'a', 30.0)"#
    );
    db.execute_unprepared(&sql).await.unwrap();

    let bucket = time_bucket_gapfill(&Interval::Hours(1), Alias::new("time"));
    let avg_expr = SimpleExpr::Custom("AVG(value)".to_string());
    let interp = interpolate(avg_expr);
    let sql = format!(
        r#"SELECT {bucket} AS bucket, {interp} AS interp_val
           FROM "{table}"
           WHERE time >= '2024-01-01 00:00:00+00' AND time < '2024-01-01 03:00:00+00'
           GROUP BY bucket ORDER BY bucket"#,
        bucket = expr_sql(&bucket),
        interp = expr_sql(&interp)
    );
    let rows = db
        .query_all(Statement::from_string(DbBackend::Postgres, sql))
        .await
        .expect("interpolate query failed");

    // 3 buckets: 00:00, 01:00, 02:00
    assert_eq!(rows.len(), 3, "Expected 3 hourly buckets with gapfill");

    // The middle bucket (hour 1) should be interpolated to 20.0
    let val: Option<f64> = rows[1].try_get("", "interp_val").unwrap();
    assert_eq!(val, Some(20.0), "interpolate should linearly fill the gap");

    drop_table(&db, &table).await;
}

#[tokio::test]
#[ignore]
async fn test_time_bucket_with_origin_query() {
    let db = connect_db().await;
    let table = setup_hypertable_with_data(&db).await;

    let bucket = time_bucket_with_origin(
        &Interval::Hours(1),
        Alias::new("time"),
        "2024-01-01 00:30:00+00",
    );
    let sql = format!(
        r#"SELECT {bucket} AS bucket, COUNT(*) AS cnt
           FROM "{table}" GROUP BY bucket ORDER BY bucket"#,
        bucket = expr_sql(&bucket)
    );
    let rows = db
        .query_all(Statement::from_string(DbBackend::Postgres, sql))
        .await
        .expect("time_bucket_with_origin query failed");

    // With origin at 00:30, buckets are [00:30-01:30), [01:30-02:30), etc.
    // Data: 00:10 (before origin bucket), 00:40 (in [00:30,01:30)), 01:10 (in [00:30,01:30))
    // 01:40 (in [01:30,02:30)), 02:10 (in [01:30,02:30))
    assert!(
        !rows.is_empty(),
        "Should return buckets with shifted alignment"
    );

    drop_table(&db, &table).await;
}

#[tokio::test]
#[ignore]
async fn test_time_bucket_with_offset_query() {
    let db = connect_db().await;
    let table = setup_hypertable_with_data(&db).await;

    let bucket = time_bucket_with_offset(
        &Interval::Hours(1),
        Alias::new("time"),
        &Interval::Minutes(30),
    );
    let sql = format!(
        r#"SELECT {bucket} AS bucket, COUNT(*) AS cnt
           FROM "{table}" GROUP BY bucket ORDER BY bucket"#,
        bucket = expr_sql(&bucket)
    );
    let rows = db
        .query_all(Statement::from_string(DbBackend::Postgres, sql))
        .await
        .expect("time_bucket_with_offset query failed");

    assert!(
        !rows.is_empty(),
        "Should return buckets with offset boundaries"
    );

    drop_table(&db, &table).await;
}

#[tokio::test]
#[ignore]
async fn test_time_bucket_tz_query() {
    let db = connect_db().await;
    let table = setup_hypertable_with_data(&db).await;

    let bucket = time_bucket_tz(&Interval::Days(1), Alias::new("time"), "UTC");
    let sql = format!(
        r#"SELECT {bucket} AS bucket, COUNT(*) AS cnt
           FROM "{table}" GROUP BY bucket ORDER BY bucket"#,
        bucket = expr_sql(&bucket)
    );
    let rows = db
        .query_all(Statement::from_string(DbBackend::Postgres, sql))
        .await
        .expect("time_bucket_tz query failed");

    // All data is on 2024-01-01 UTC, so expect 1 daily bucket
    assert_eq!(rows.len(), 1, "Expected 1 daily bucket in UTC");

    drop_table(&db, &table).await;
}

#[tokio::test]
#[ignore]
async fn test_remove_retention_policy() {
    let db = connect_db().await;
    let table = setup_hypertable_with_data(&db).await;

    add_retention_policy(
        &db,
        &table,
        &RetentionConfig {
            drop_after: Interval::Days(365),
        },
    )
    .await
    .expect("add_retention_policy should succeed");

    remove_retention_policy(&db, &table)
        .await
        .expect("remove_retention_policy should succeed");

    drop_table(&db, &table).await;
}

#[tokio::test]
#[ignore]
async fn test_remove_compression_policy() {
    let db = connect_db().await;
    let table = setup_hypertable_with_data(&db).await;

    enable_compression(
        &db,
        &table,
        &CompressionConfig {
            segment_by: vec!["sensor_id".into()],
            order_by: vec![("time".into(), SortDirection::Desc)],
            compress_after: Interval::Days(30),
        },
    )
    .await
    .expect("enable_compression should succeed");

    remove_compression_policy(&db, &table)
        .await
        .expect("remove_compression_policy should succeed");

    drop_table(&db, &table).await;
}

#[tokio::test]
#[ignore]
async fn test_remove_continuous_aggregate_policy() {
    let db = connect_db().await;
    let table = setup_hypertable_with_data(&db).await;
    let view = unique_name("cagg");

    let select_sql = format!(
        r#"SELECT time_bucket('1 hour', time) AS bucket,
                  sensor_id,
                  AVG(value) AS avg_value
           FROM "{table}"
           GROUP BY bucket, sensor_id"#
    );

    create_continuous_aggregate(
        &db,
        &select_sql,
        &ContinuousAggregateConfig {
            view_name: view.clone(),
            bucket_interval: Interval::Hours(1),
            refresh_policy: Some(RefreshPolicy {
                start_offset: Interval::Days(3),
                end_offset: Interval::Hours(1),
                schedule_interval: Interval::Hours(1),
            }),
        },
    )
    .await
    .expect("create_continuous_aggregate should succeed");

    remove_continuous_aggregate_policy(&db, &view)
        .await
        .expect("remove_continuous_aggregate_policy should succeed");

    drop_view(&db, &view).await;
    drop_table(&db, &table).await;
}

#[tokio::test]
#[ignore]
async fn test_drop_chunks() {
    let db = connect_db().await;
    let table = unique_name("ts");
    create_test_table(&db, &table).await;

    create_hypertable(
        &db,
        &HypertableConfig {
            table_name: table.clone(),
            time_column: "time".into(),
            chunk_interval: Some(Interval::Days(1)),
            if_not_exists: true,
        },
    )
    .await
    .unwrap();

    // Insert old data (30+ days ago) and recent data
    let sql = format!(
        r#"INSERT INTO "{table}" (time, sensor_id, value) VALUES
            ('2020-01-01 00:00:00+00', 'a', 1.0),
            ('2020-01-02 00:00:00+00', 'a', 2.0),
            (NOW(), 'a', 100.0)"#
    );
    db.execute_unprepared(&sql).await.unwrap();

    // Count before dropping
    let before = db
        .query_all(Statement::from_string(
            DbBackend::Postgres,
            format!(r#"SELECT * FROM "{table}""#),
        ))
        .await
        .unwrap();
    assert_eq!(before.len(), 3);

    // Drop chunks older than 30 days
    drop_chunks(&db, &table, &Interval::Days(30))
        .await
        .expect("drop_chunks should succeed");

    // Count after dropping — old data should be gone
    let after = db
        .query_all(Statement::from_string(
            DbBackend::Postgres,
            format!(r#"SELECT * FROM "{table}""#),
        ))
        .await
        .unwrap();
    assert_eq!(
        after.len(),
        1,
        "Only recent data should remain after drop_chunks"
    );

    drop_table(&db, &table).await;
}

#[tokio::test]
#[ignore]
async fn test_invalid_identifiers() {
    let db = connect_db().await;

    // create_hypertable rejects SQL injection in table name
    let result = create_hypertable(
        &db,
        &HypertableConfig {
            table_name: "table; DROP TABLE users".into(),
            time_column: "time".into(),
            chunk_interval: None,
            if_not_exists: false,
        },
    )
    .await;

    assert!(result.is_err(), "Should reject invalid table name");
    match result.unwrap_err() {
        DbErr::Custom(msg) => {
            assert!(
                msg.contains("Invalid SQL identifier"),
                "Unexpected error: {msg}"
            );
        }
        other => panic!("Expected DbErr::Custom, got: {other:?}"),
    }

    // enable_compression rejects bad column names
    let table = setup_hypertable_with_data(&db).await;
    let result = enable_compression(
        &db,
        &table,
        &CompressionConfig {
            segment_by: vec!["bad column!".into()],
            order_by: vec![],
            compress_after: Interval::Days(30),
        },
    )
    .await;

    assert!(result.is_err(), "Should reject invalid column name");
    match result.unwrap_err() {
        DbErr::Custom(msg) => {
            assert!(
                msg.contains("Invalid SQL identifier"),
                "Unexpected error: {msg}"
            );
        }
        other => panic!("Expected DbErr::Custom, got: {other:?}"),
    }

    drop_table(&db, &table).await;
}
