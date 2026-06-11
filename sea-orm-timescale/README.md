# sea-orm-timescale

Cloned from <https://github.com/evanjt/sea-orm-timescale>

[TimescaleDB](https://www.timescale.com/) functions and migration helpers for [Sea-ORM](https://www.sea-ql.org/SeaORM/).

```bash
cargo add sea-orm-timescale
```

## Usage

Query helper functions return `SimpleExpr` values that slot into Sea-ORM's query builder:

```rust
use sea_orm::entity::prelude::*;
use sea_orm_timescale::{functions::time_bucket, types::Interval};

let hourly = readings::Entity::find()
    .select_only()
    .column_as(
        time_bucket(&Interval::Hours(1), readings::Column::Time),
        "bucket",
    )
    .column_as(readings::Column::Value.avg(), "avg_value")
    .group_by(time_bucket(&Interval::Hours(1), readings::Column::Time))
    .into_json()
    .all(&db)
    .await?;
```

Also provides `first`, `last`, `locf`, `time_bucket_gapfill`, and `histogram` — see [docs.rs](https://docs.rs/sea-orm-timescale) for the full API.

## Migrations

Helpers for common TimescaleDB DDL operations. Use these in your Sea-ORM migrations:

```rust
use sea_orm_timescale::migration::*;
use sea_orm_timescale::types::*;

// Convert a table to a hypertable
create_hypertable(&db, &HypertableConfig {
    table_name: "readings".into(),
    time_column: "time".into(),
    chunk_interval: Some(Interval::Days(7)),
    if_not_exists: true,
}).await?;

// Enable compression with a 30-day policy
enable_compression(&db, "readings", &CompressionConfig {
    segment_by: vec!["site_id".into()],
    order_by: vec![("time".into(), SortDirection::Desc)],
    compress_after: Interval::Days(30),
}).await?;

// Drop chunks older than 1 year
add_retention_policy(&db, "readings", &RetentionConfig {
    drop_after: Interval::Days(365),
}).await?;
```

Continuous aggregates and refresh are also supported — see [docs.rs](https://docs.rs/sea-orm-timescale).

## License

MIT
