//! Coverage for route submission age bucket helper logic.

use chrono::{Duration, TimeZone, Utc};
use rstest::rstest;

use super::super::calculate_age_bucket;

/// Fixed reference time for deterministic tests.
fn fixed_now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2025, 1, 15, 12, 0, 0).unwrap()
}

/// Parameterized test for age bucket boundary values.
///
/// Tests cover all bucket boundaries and edge cases:
/// - `0-1m`: 0 to 59 seconds
/// - `1-5m`: 1 to 4 minutes
/// - `5-30m`: 5 to 29 minutes
/// - `30m-2h`: 30 to 119 minutes
/// - `2h-6h`: 120 to 359 minutes
/// - `6h-24h`: 360 to 1439 minutes
/// - `>24h`: 1440+ minutes
/// - Future timestamps (clock skew) clamp to `0-1m`
#[rstest]
#[case::zero_seconds(0, "0-1m")]
#[case::thirty_seconds(30, "0-1m")]
#[case::one_minute(60, "1-5m")]
#[case::four_minutes(4 * 60, "1-5m")]
#[case::five_minutes(5 * 60, "5-30m")]
#[case::twenty_nine_minutes(29 * 60, "5-30m")]
#[case::thirty_minutes(30 * 60, "30m-2h")]
#[case::one_hour(60 * 60, "30m-2h")]
#[case::two_hours(2 * 60 * 60, "2h-6h")]
#[case::five_hours(5 * 60 * 60, "2h-6h")]
#[case::six_hours(6 * 60 * 60, "6h-24h")]
#[case::twenty_three_hours(23 * 60 * 60, "6h-24h")]
#[case::twenty_four_hours(24 * 60 * 60, ">24h")]
#[case::forty_eight_hours(48 * 60 * 60, ">24h")]
#[case::future_timestamp_clamps(-5 * 60, "0-1m")]
fn age_bucket_boundaries(#[case] offset_seconds: i64, #[case] expected: &str) {
    let now = fixed_now();
    let created = now - Duration::seconds(offset_seconds);
    assert_eq!(calculate_age_bucket(created, now), expected);
}
