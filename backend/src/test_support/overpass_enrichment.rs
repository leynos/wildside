//! Shared test doubles for Overpass enrichment worker tests.

use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Local, TimeDelta, Utc};
use mockable::Clock;

use crate::domain::overpass_enrichment_worker::{BackoffJitter, EnrichmentSleeper};

/// Mutable test clock backed by a mutex-protected UTC instant.
///
/// This helper is thread-safe and can be shared across async tasks in tests.
///
/// # Examples
///
/// ```rust
/// use backend::test_support::overpass_enrichment::MutableClock;
/// use chrono::{TimeZone, Utc};
/// use mockable::Clock;
/// use std::time::Duration;
///
/// let initial = Utc
///     .with_ymd_and_hms(2026, 2, 26, 12, 0, 0)
///     .single()
///     .expect("valid time");
/// let clock = MutableClock::new(initial);
///
/// clock.advance(Duration::from_secs(5));
/// assert_eq!(
///     clock.utc(),
///     Utc.with_ymd_and_hms(2026, 2, 26, 12, 0, 5)
///         .single()
///         .expect("valid time"),
/// );
/// ```
pub struct MutableClock(Mutex<DateTime<Utc>>);

impl MutableClock {
    /// Create a clock pinned to the provided UTC instant.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use backend::test_support::overpass_enrichment::MutableClock;
    /// use chrono::{TimeZone, Utc};
    /// use mockable::Clock;
    ///
    /// let now = Utc
    ///     .with_ymd_and_hms(2026, 2, 26, 12, 0, 0)
    ///     .single()
    ///     .expect("valid time");
    /// let clock = MutableClock::new(now);
    /// assert_eq!(clock.utc(), now);
    /// ```
    pub fn new(now: DateTime<Utc>) -> Self {
        Self(Mutex::new(now))
    }

    /// Advance the clock by a `Duration`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use backend::test_support::overpass_enrichment::MutableClock;
    /// use chrono::{TimeZone, Utc};
    /// use mockable::Clock;
    /// use std::time::Duration;
    ///
    /// let clock = MutableClock::new(
    ///     Utc.with_ymd_and_hms(2026, 2, 26, 12, 0, 0)
    ///         .single()
    ///         .expect("valid time"),
    /// );
    /// clock.advance(Duration::from_millis(250));
    /// assert_eq!(
    ///     clock.utc(),
    ///     Utc.with_ymd_and_hms(2026, 2, 26, 12, 0, 0)
    ///         .single()
    ///         .expect("valid time")
    ///         + chrono::TimeDelta::milliseconds(250),
    /// );
    /// ```
    pub fn advance(&self, delta: Duration) {
        let delta = match TimeDelta::from_std(delta) {
            Ok(delta) => delta,
            Err(error) => {
                panic!("failed to convert Duration to TimeDelta: {error}; delta={delta:?}",)
            }
        };
        *self.lock_clock() += delta;
    }

    /// Advance the clock by a whole number of seconds.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use backend::test_support::overpass_enrichment::MutableClock;
    /// use chrono::{TimeZone, Utc};
    /// use mockable::Clock;
    ///
    /// let clock = MutableClock::new(
    ///     Utc.with_ymd_and_hms(2026, 2, 26, 12, 0, 0)
    ///         .single()
    ///         .expect("valid time"),
    /// );
    /// clock.advance_seconds(3);
    /// assert_eq!(
    ///     clock.utc(),
    ///     Utc.with_ymd_and_hms(2026, 2, 26, 12, 0, 3)
    ///         .single()
    ///         .expect("valid time"),
    /// );
    /// ```
    pub fn advance_seconds(&self, seconds: i64) {
        *self.lock_clock() += TimeDelta::seconds(seconds);
    }

    fn lock_clock(&self) -> std::sync::MutexGuard<'_, DateTime<Utc>> {
        match self.0.lock() {
            Ok(guard) => guard,
            Err(error) => panic!("clock mutex lock failed: {error:?}"),
        }
    }
}

impl Clock for MutableClock {
    /// Return the clock value converted to local timezone.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use backend::test_support::overpass_enrichment::MutableClock;
    /// use chrono::{TimeZone, Utc};
    /// use mockable::Clock;
    ///
    /// let now = Utc
    ///     .with_ymd_and_hms(2026, 2, 26, 12, 0, 0)
    ///     .single()
    ///     .expect("valid time");
    /// let clock = MutableClock::new(now);
    /// let local = clock.local();
    /// assert_eq!(local.with_timezone(&Utc), now);
    /// ```
    fn local(&self) -> DateTime<Local> {
        self.utc().with_timezone(&Local)
    }

    /// Return the current UTC instant stored in the clock.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use backend::test_support::overpass_enrichment::MutableClock;
    /// use chrono::{TimeZone, Utc};
    /// use mockable::Clock;
    ///
    /// let now = Utc
    ///     .with_ymd_and_hms(2026, 2, 26, 12, 0, 0)
    ///     .single()
    ///     .expect("valid time");
    /// let clock = MutableClock::new(now);
    /// assert_eq!(clock.utc(), now);
    /// ```
    fn utc(&self) -> DateTime<Utc> {
        *self.lock_clock()
    }
}

/// Sleeper test double that returns immediately.
///
/// # Examples
///
/// ```rust,no_run
/// use backend::domain::EnrichmentSleeper;
/// use backend::test_support::overpass_enrichment::ImmediateSleeper;
/// use std::time::Duration;
///
/// # async fn demo() {
/// let sleeper = ImmediateSleeper;
/// sleeper.sleep(Duration::from_millis(10)).await;
/// # }
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct ImmediateSleeper;

#[async_trait]
impl EnrichmentSleeper for ImmediateSleeper {
    /// Complete immediately without waiting.
    async fn sleep(&self, _duration: Duration) {}
}

/// Sleeper test double that records requested delays in call order.
///
/// # Examples
///
/// ```rust,no_run
/// use backend::domain::EnrichmentSleeper;
/// use backend::test_support::overpass_enrichment::RecordingSleeper;
/// use std::time::Duration;
///
/// # async fn demo() {
/// let sleeper = RecordingSleeper::default();
/// sleeper.sleep(Duration::from_millis(5)).await;
/// sleeper.sleep(Duration::from_millis(10)).await;
///
/// assert_eq!(
///     sleeper.0.lock().expect("sleeper mutex").as_slice(),
///     [Duration::from_millis(5), Duration::from_millis(10)],
/// );
/// # }
/// ```
#[derive(Default)]
pub struct RecordingSleeper(pub Mutex<Vec<Duration>>);

#[async_trait]
impl EnrichmentSleeper for RecordingSleeper {
    /// Record the requested sleep duration.
    async fn sleep(&self, duration: Duration) {
        let mut entries = match self.0.lock() {
            Ok(entries) => entries,
            Err(error) => panic!("sleeper mutex lock failed: {error:?}"),
        };
        entries.push(duration);
    }
}

/// Jitter strategy that always returns the base delay unchanged.
///
/// # Examples
///
/// ```rust
/// use backend::domain::BackoffJitter;
/// use backend::test_support::overpass_enrichment::NoJitter;
/// use chrono::{TimeZone, Utc};
/// use std::time::Duration;
///
/// let jitter = NoJitter;
/// let now = Utc
///     .with_ymd_and_hms(2026, 2, 26, 12, 0, 0)
///     .single()
///     .expect("valid time");
/// assert_eq!(
///     jitter.jittered_delay(Duration::from_millis(100), 3, now),
///     Duration::from_millis(100),
/// );
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct NoJitter;

impl BackoffJitter for NoJitter {
    /// Return `base` without modification.
    fn jittered_delay(&self, base: Duration, _attempt: u32, _now: DateTime<Utc>) -> Duration {
        base
    }
}

/// Deterministic jitter strategy that offsets delay by `attempt` milliseconds.
///
/// # Examples
///
/// ```rust
/// use backend::domain::BackoffJitter;
/// use backend::test_support::overpass_enrichment::AttemptOffsetJitter;
/// use chrono::{TimeZone, Utc};
/// use std::time::Duration;
///
/// let jitter = AttemptOffsetJitter;
/// let now = Utc
///     .with_ymd_and_hms(2026, 2, 26, 12, 0, 0)
///     .single()
///     .expect("valid time");
/// assert_eq!(
///     jitter.jittered_delay(Duration::from_millis(100), 2, now),
///     Duration::from_millis(102),
/// );
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct AttemptOffsetJitter;

impl BackoffJitter for AttemptOffsetJitter {
    /// Return `base + attempt(ms)` to keep retry timing deterministic in tests.
    fn jittered_delay(&self, base: Duration, attempt: u32, _now: DateTime<Utc>) -> Duration {
        base + Duration::from_millis(u64::from(attempt))
    }
}
