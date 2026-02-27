//! Shared test doubles for Overpass enrichment worker tests.

use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Local, TimeDelta, Utc};
use mockable::Clock;

use crate::domain::overpass_enrichment_worker::{BackoffJitter, EnrichmentSleeper};

pub struct MutableClock(Mutex<DateTime<Utc>>);

impl MutableClock {
    pub fn new(now: DateTime<Utc>) -> Self {
        Self(Mutex::new(now))
    }

    pub fn advance(&self, delta: Duration) {
        let delta = match TimeDelta::from_std(delta) {
            Ok(delta) => delta,
            Err(error) => {
                panic!("failed to convert Duration to TimeDelta: {error}; delta={delta:?}",)
            }
        };
        *self.lock_clock() += delta;
    }

    pub fn advance_seconds(&self, seconds: i64) {
        *self.lock_clock() += TimeDelta::seconds(seconds);
    }

    fn lock_clock(&self) -> std::sync::MutexGuard<'_, DateTime<Utc>> {
        match self.0.lock() {
            Ok(guard) => guard,
            Err(_) => panic!("clock mutex"),
        }
    }
}

impl Clock for MutableClock {
    fn local(&self) -> DateTime<Local> {
        self.utc().with_timezone(&Local)
    }

    fn utc(&self) -> DateTime<Utc> {
        *self.lock_clock()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ImmediateSleeper;

#[async_trait]
impl EnrichmentSleeper for ImmediateSleeper {
    async fn sleep(&self, _duration: Duration) {}
}

#[derive(Default)]
pub struct RecordingSleeper(pub Mutex<Vec<Duration>>);

#[async_trait]
impl EnrichmentSleeper for RecordingSleeper {
    async fn sleep(&self, duration: Duration) {
        let mut entries = match self.0.lock() {
            Ok(entries) => entries,
            Err(_) => panic!("sleeper mutex"),
        };
        entries.push(duration);
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoJitter;

impl BackoffJitter for NoJitter {
    fn jittered_delay(&self, base: Duration, _attempt: u32, _now: DateTime<Utc>) -> Duration {
        base
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AttemptOffsetJitter;

impl BackoffJitter for AttemptOffsetJitter {
    fn jittered_delay(&self, base: Duration, attempt: u32, _now: DateTime<Utc>) -> Duration {
        base + Duration::from_millis(u64::from(attempt))
    }
}
