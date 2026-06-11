use sea_orm::sea_query::{Alias, SimpleExpr};
use sea_orm_timescale::functions::*;
use sea_orm_timescale::types::*;

/// Extract the SQL string from a SimpleExpr::Custom variant.
fn custom_sql(expr: &SimpleExpr) -> &str {
    match expr {
        SimpleExpr::Custom(s) => s.as_str(),
        _ => panic!("Expected SimpleExpr::Custom"),
    }
}

#[test]
fn test_time_bucket_hours() {
    let expr = time_bucket(&Interval::Hours(1), Alias::new("time"));
    assert_eq!(custom_sql(&expr), "time_bucket('1 hours', \"time\")");
}

#[test]
fn test_time_bucket_minutes() {
    let expr = time_bucket(&Interval::Minutes(15), Alias::new("timestamp"));
    assert_eq!(
        custom_sql(&expr),
        "time_bucket('15 minutes', \"timestamp\")"
    );
}

#[test]
fn test_time_bucket_days() {
    let expr = time_bucket(&Interval::Days(7), Alias::new("created_at"));
    assert_eq!(custom_sql(&expr), "time_bucket('7 days', \"created_at\")");
}

#[test]
fn test_time_bucket_gapfill_expression() {
    let expr = time_bucket_gapfill(&Interval::Hours(1), Alias::new("time"));
    assert_eq!(
        custom_sql(&expr),
        "time_bucket_gapfill('1 hours', \"time\")"
    );
}

#[test]
fn test_first_aggregate() {
    let expr = first(Alias::new("temperature"), Alias::new("time"));
    assert_eq!(custom_sql(&expr), "first(\"temperature\", \"time\")");
}

#[test]
fn test_last_aggregate() {
    let expr = last(Alias::new("temperature"), Alias::new("time"));
    assert_eq!(custom_sql(&expr), "last(\"temperature\", \"time\")");
}

#[test]
fn test_locf_with_custom_expr() {
    let inner = SimpleExpr::Custom("AVG(\"value\")".to_string());
    let expr = locf(inner);
    assert_eq!(custom_sql(&expr), "locf(AVG(\"value\"))");
}

#[test]
fn test_histogram_expression() {
    let expr = histogram(Alias::new("temperature"), -10.0, 50.0, 20);
    assert_eq!(custom_sql(&expr), "histogram(\"temperature\", -10, 50, 20)");
}

// --- Interval parsing tests ---

#[test]
fn test_interval_parse_full_formats() {
    assert_eq!(Interval::parse("1 hour").unwrap(), Interval::Hours(1));
    assert_eq!(Interval::parse("5 minutes").unwrap(), Interval::Minutes(5));
    assert_eq!(
        Interval::parse("30 seconds").unwrap(),
        Interval::Seconds(30)
    );
    assert_eq!(Interval::parse("7 days").unwrap(), Interval::Days(7));
    assert_eq!(Interval::parse("1 week").unwrap(), Interval::Weeks(1));
    assert_eq!(Interval::parse("3 months").unwrap(), Interval::Months(3));
}

#[test]
fn test_interval_parse_short_formats() {
    assert_eq!(Interval::parse("1h").unwrap(), Interval::Hours(1));
    assert_eq!(Interval::parse("5m").unwrap(), Interval::Minutes(5));
    assert_eq!(Interval::parse("30s").unwrap(), Interval::Seconds(30));
    assert_eq!(Interval::parse("7d").unwrap(), Interval::Days(7));
    assert_eq!(Interval::parse("1w").unwrap(), Interval::Weeks(1));
    assert_eq!(Interval::parse("1M").unwrap(), Interval::Months(1));
}

#[test]
fn test_interval_parse_aliases() {
    assert_eq!(Interval::parse("2 hrs").unwrap(), Interval::Hours(2));
    assert_eq!(Interval::parse("10 mins").unwrap(), Interval::Minutes(10));
    assert_eq!(Interval::parse("60 secs").unwrap(), Interval::Seconds(60));
}

#[test]
fn test_interval_display() {
    assert_eq!(Interval::Hours(1).to_string(), "1 hours");
    assert_eq!(Interval::Minutes(30).to_string(), "30 minutes");
    assert_eq!(Interval::Days(7).to_string(), "7 days");
    assert_eq!(Interval::Months(3).to_string(), "3 months");
}

#[test]
fn test_interval_parse_errors() {
    assert!(Interval::parse("").is_err());
    assert!(Interval::parse("abc").is_err());
    assert!(Interval::parse("1 lightyear").is_err());
}

#[test]
fn test_interpolate_expression() {
    let inner = SimpleExpr::Custom("AVG(\"value\")".to_string());
    let expr = interpolate(inner);
    assert_eq!(custom_sql(&expr), "interpolate(AVG(\"value\"))");
}

#[test]
fn test_time_bucket_with_origin_expression() {
    let expr = time_bucket_with_origin(
        &Interval::Hours(1),
        Alias::new("time"),
        "2024-01-01 00:00:00+00",
    );
    assert_eq!(
        custom_sql(&expr),
        "time_bucket('1 hours', \"time\", origin => '2024-01-01 00:00:00+00')"
    );
}

#[test]
fn test_time_bucket_with_offset_expression() {
    let expr = time_bucket_with_offset(
        &Interval::Hours(1),
        Alias::new("time"),
        &Interval::Minutes(30),
    );
    assert_eq!(
        custom_sql(&expr),
        "time_bucket('1 hours', \"time\", INTERVAL '30 minutes')"
    );
}

#[test]
fn test_time_bucket_tz_expression() {
    let expr = time_bucket_tz(&Interval::Days(1), Alias::new("time"), "US/Eastern");
    assert_eq!(
        custom_sql(&expr),
        "time_bucket('1 days', \"time\", timezone => 'US/Eastern')"
    );
}

#[test]
fn test_interval_to_sql() {
    let interval = Interval::Hours(6);
    assert_eq!(interval.to_sql_interval(), "6 hours");
}
