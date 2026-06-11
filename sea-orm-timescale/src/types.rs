use std::fmt;

/// A PostgreSQL interval for use in `time_bucket`, retention policies, compression, etc.
///
/// Renders to SQL interval literals like `'7 days'` or `'1 hours'`.
/// Can also be parsed from strings via [`Interval::parse`].
///
/// # Example
/// ```
/// use sea_orm_timescale::types::Interval;
///
/// let weekly = Interval::Days(7);
/// assert_eq!(weekly.to_string(), "7 days");
///
/// let parsed = Interval::parse("1h").unwrap();
/// assert_eq!(parsed, Interval::Hours(1));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Interval {
    /// Microseconds (e.g. `500 microseconds`).
    Microseconds(i64),
    /// Milliseconds (e.g. `100 milliseconds`).
    Milliseconds(i64),
    /// Seconds (e.g. `30 seconds`). Short form: `"30s"`.
    Seconds(i64),
    /// Minutes (e.g. `5 minutes`). Short form: `"5m"`.
    Minutes(i64),
    /// Hours (e.g. `1 hours`). Short form: `"1h"`.
    Hours(i64),
    /// Days (e.g. `7 days`). Short form: `"7d"`.
    Days(i64),
    /// Weeks (e.g. `1 weeks`). Short form: `"1w"`.
    Weeks(i64),
    /// Months (e.g. `3 months`). Short form: `"3M"` (uppercase M).
    Months(i64),
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Microseconds(n) => write!(f, "{n} microseconds"),
            Self::Milliseconds(n) => write!(f, "{n} milliseconds"),
            Self::Seconds(n) => write!(f, "{n} seconds"),
            Self::Minutes(n) => write!(f, "{n} minutes"),
            Self::Hours(n) => write!(f, "{n} hours"),
            Self::Days(n) => write!(f, "{n} days"),
            Self::Weeks(n) => write!(f, "{n} weeks"),
            Self::Months(n) => write!(f, "{n} months"),
        }
    }
}

impl Interval {
    /// Parse a human-readable interval string like "1 hour", "5 minutes", "1h", "30m".
    ///
    /// Supported formats:
    /// - Full: "1 hour", "5 minutes", "7 days"
    /// - Short: "1h", "5m", "7d", "1w", "30s"
    /// - Plural: "2 hours", "10 days"
    pub fn parse(s: &str) -> Result<Self, IntervalParseError> {
        let s = s.trim();

        // Try short format: "1h", "5m", "30s", "7d", "1w"
        if let Some(result) = Self::try_parse_short(s) {
            return result;
        }

        // Try full format: "1 hour", "5 minutes"
        let parts: Vec<&str> = s.splitn(2, ' ').collect();
        if parts.len() != 2 {
            return Err(IntervalParseError::InvalidFormat(s.to_string()));
        }

        let value: i64 = parts[0]
            .parse()
            .map_err(|_| IntervalParseError::InvalidNumber(parts[0].to_string()))?;

        let unit = parts[1].trim().to_lowercase();
        match unit.as_str() {
            "microsecond" | "microseconds" | "us" => Ok(Self::Microseconds(value)),
            "millisecond" | "milliseconds" | "ms" => Ok(Self::Milliseconds(value)),
            "second" | "seconds" | "sec" | "secs" => Ok(Self::Seconds(value)),
            "minute" | "minutes" | "min" | "mins" => Ok(Self::Minutes(value)),
            "hour" | "hours" | "hr" | "hrs" => Ok(Self::Hours(value)),
            "day" | "days" => Ok(Self::Days(value)),
            "week" | "weeks" => Ok(Self::Weeks(value)),
            "month" | "months" | "mon" | "mons" => Ok(Self::Months(value)),
            _ => Err(IntervalParseError::UnknownUnit(unit)),
        }
    }

    fn try_parse_short(s: &str) -> Option<Result<Self, IntervalParseError>> {
        // Must end with a letter and have digits before it
        let boundary = s.find(|c: char| c.is_ascii_alphabetic())?;
        if boundary == 0 {
            return None;
        }

        let (num_str, unit_str) = s.split_at(boundary);

        // Only match single-char short codes to avoid ambiguity with full format
        if unit_str.len() > 2 || unit_str.contains(' ') {
            return None;
        }

        let value: i64 = match num_str.parse() {
            Ok(v) => v,
            Err(_) => return Some(Err(IntervalParseError::InvalidNumber(num_str.to_string()))),
        };

        match unit_str {
            "us" => Some(Ok(Self::Microseconds(value))),
            "ms" => Some(Ok(Self::Milliseconds(value))),
            "s" => Some(Ok(Self::Seconds(value))),
            "m" => Some(Ok(Self::Minutes(value))),
            "h" => Some(Ok(Self::Hours(value))),
            "d" => Some(Ok(Self::Days(value))),
            "w" => Some(Ok(Self::Weeks(value))),
            "M" => Some(Ok(Self::Months(value))),
            _ => None,
        }
    }

    /// Return the SQL interval literal string for use in TimescaleDB functions.
    pub fn to_sql_interval(&self) -> String {
        self.to_string()
    }
}

/// Error returned by [`Interval::parse`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntervalParseError {
    /// Input couldn't be split into a number and unit (e.g. empty string).
    InvalidFormat(String),
    /// The numeric portion couldn't be parsed as an integer.
    InvalidNumber(String),
    /// The unit portion wasn't recognised (e.g. `"lightyear"`).
    UnknownUnit(String),
}

impl fmt::Display for IntervalParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat(s) => write!(f, "invalid interval format: '{s}'"),
            Self::InvalidNumber(s) => write!(f, "invalid number in interval: '{s}'"),
            Self::UnknownUnit(s) => write!(f, "unknown interval unit: '{s}'"),
        }
    }
}

impl std::error::Error for IntervalParseError {}

/// Sort direction for compression ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl fmt::Display for SortDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asc => write!(f, "ASC"),
            Self::Desc => write!(f, "DESC"),
        }
    }
}

/// Configuration for [`create_hypertable`](crate::migration::create_hypertable).
#[derive(Debug, Clone)]
pub struct HypertableConfig {
    /// The existing table to convert (e.g. `"readings"`).
    pub table_name: String,
    /// The `TIMESTAMPTZ` column used as the time dimension.
    pub time_column: String,
    /// Chunk size for partitioning. Defaults to 7 days if `None`.
    pub chunk_interval: Option<Interval>,
    /// When `true`, silently succeeds if the hypertable already exists.
    pub if_not_exists: bool,
}

/// Configuration for [`enable_compression`](crate::migration::enable_compression).
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    /// Columns to segment compressed data by (e.g. `["site_id"]`).
    pub segment_by: Vec<String>,
    /// Columns to order compressed data by within each segment.
    pub order_by: Vec<(String, SortDirection)>,
    /// Automatically compress chunks older than this interval.
    pub compress_after: Interval,
}

/// Configuration for [`create_continuous_aggregate`](crate::migration::create_continuous_aggregate).
#[derive(Debug, Clone)]
pub struct ContinuousAggregateConfig {
    /// Name for the materialized view (e.g. `"readings_hourly"`).
    pub view_name: String,
    /// The bucket interval used in the aggregate's `time_bucket` call.
    pub bucket_interval: Interval,
    /// Optional automatic refresh policy. If `None`, the aggregate must be refreshed manually.
    pub refresh_policy: Option<RefreshPolicy>,
}

/// Automatic refresh policy for a continuous aggregate.
///
/// TimescaleDB will run a background job on `schedule_interval` to refresh
/// data between `start_offset` and `end_offset` relative to now.
#[derive(Debug, Clone)]
pub struct RefreshPolicy {
    /// How far back from now to start refreshing (e.g. `Interval::Days(3)`).
    pub start_offset: Interval,
    /// How close to now to stop refreshing (e.g. `Interval::Hours(1)`).
    pub end_offset: Interval,
    /// How often to run the refresh job.
    pub schedule_interval: Interval,
}

/// Configuration for [`add_retention_policy`](crate::migration::add_retention_policy).
#[derive(Debug, Clone)]
pub struct RetentionConfig {
    /// Automatically drop chunks older than this interval.
    pub drop_after: Interval,
}
