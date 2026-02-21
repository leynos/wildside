//! Builder-style test parameters for walk-session integration fixtures.

use backend::domain::{
    UserId, WalkPrimaryStat, WalkPrimaryStatKind, WalkSecondaryStat, WalkSecondaryStatKind,
    WalkSession, WalkSessionDraft,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Value object for walk-stat overrides in integration fixtures.
///
/// # Examples
///
/// ```no_run
/// let stats = WalkSessionStats::new(4200.0, 3600.0, 410.0, 18.0);
/// let _ = stats;
/// ```
pub(crate) struct WalkSessionStats {
    distance: f64,
    duration: f64,
    energy: f64,
    poi_count: f64,
}

impl WalkSessionStats {
    /// Create a test stats payload.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let stats = WalkSessionStats::new(2500.0, 1800.0, 260.0, 9.0);
    /// let _ = stats;
    /// ```
    pub(crate) fn new(distance: f64, duration: f64, energy: f64, poi_count: f64) -> Self {
        Self {
            distance,
            duration,
            energy,
            poi_count,
        }
    }
}

/// Builder for `WalkSession` fixtures used by repository integration tests.
///
/// # Examples
///
/// ```no_run
/// use backend::domain::UserId;
/// use chrono::Utc;
/// use uuid::Uuid;
///
/// let params = WalkSessionTestParams::new(UserId::random(), Uuid::new_v4(), Utc::now());
/// let session = params.build();
/// assert!(session.ended_at().is_none());
/// ```
pub(crate) struct WalkSessionTestParams {
    id: Uuid,
    user_id: UserId,
    route_id: Uuid,
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
    distance: f64,
    duration: f64,
    energy: f64,
    poi_count: f64,
}

impl WalkSessionTestParams {
    /// Start a builder with default stats and no completion timestamp.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use backend::domain::UserId;
    /// use chrono::Utc;
    /// use uuid::Uuid;
    ///
    /// let params = WalkSessionTestParams::new(UserId::random(), Uuid::new_v4(), Utc::now());
    /// let _ = params;
    /// ```
    pub(crate) fn new(user_id: UserId, route_id: Uuid, started_at: DateTime<Utc>) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            route_id,
            started_at,
            ended_at: None,
            distance: 3650.0,
            duration: 2820.0,
            energy: 320.0,
            poi_count: 12.0,
        }
    }

    /// Override the fixture id, typically for upsert tests.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use backend::domain::UserId;
    /// use chrono::Utc;
    /// use uuid::Uuid;
    ///
    /// let fixed_id = Uuid::new_v4();
    /// let params = WalkSessionTestParams::new(UserId::random(), Uuid::new_v4(), Utc::now())
    ///     .with_id(fixed_id);
    /// let _ = params;
    /// ```
    pub(crate) fn with_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    /// Mark the fixture as completed at the provided timestamp.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use backend::domain::UserId;
    /// use chrono::{Duration, Utc};
    /// use uuid::Uuid;
    ///
    /// let started_at = Utc::now();
    /// let params = WalkSessionTestParams::new(UserId::random(), Uuid::new_v4(), started_at)
    ///     .with_ended_at(started_at + Duration::minutes(30));
    /// let _ = params;
    /// ```
    pub(crate) fn with_ended_at(mut self, ended_at: DateTime<Utc>) -> Self {
        self.ended_at = Some(ended_at);
        self
    }

    /// Replace default stat values with a custom set.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use backend::domain::UserId;
    /// use chrono::Utc;
    /// use uuid::Uuid;
    ///
    /// let stats = WalkSessionStats::new(1200.0, 900.0, 120.0, 4.0);
    /// let params = WalkSessionTestParams::new(UserId::random(), Uuid::new_v4(), Utc::now())
    ///     .with_stats(stats);
    /// let _ = params;
    /// ```
    pub(crate) fn with_stats(mut self, stats: WalkSessionStats) -> Self {
        self.distance = stats.distance;
        self.duration = stats.duration;
        self.energy = stats.energy;
        self.poi_count = stats.poi_count;
        self
    }

    /// Build a fully validated `WalkSession` fixture.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use backend::domain::UserId;
    /// use chrono::Utc;
    /// use uuid::Uuid;
    ///
    /// let session = WalkSessionTestParams::new(UserId::random(), Uuid::new_v4(), Utc::now())
    ///     .with_stats(WalkSessionStats::new(2500.0, 1800.0, 260.0, 9.0))
    ///     .build();
    ///
    /// assert_eq!(session.primary_stats().len(), 2);
    /// assert_eq!(session.secondary_stats().len(), 2);
    /// ```
    pub(crate) fn build(self) -> WalkSession {
        WalkSession::new(WalkSessionDraft {
            id: self.id,
            user_id: self.user_id,
            route_id: self.route_id,
            started_at: self.started_at,
            ended_at: self.ended_at,
            primary_stats: vec![
                WalkPrimaryStat::new(WalkPrimaryStatKind::Distance, self.distance)
                    .expect("valid distance stat"),
                WalkPrimaryStat::new(WalkPrimaryStatKind::Duration, self.duration)
                    .expect("valid duration stat"),
            ],
            secondary_stats: vec![
                WalkSecondaryStat::new(
                    WalkSecondaryStatKind::Energy,
                    self.energy,
                    Some("kcal".to_owned()),
                )
                .expect("valid energy stat"),
                WalkSecondaryStat::new(WalkSecondaryStatKind::Count, self.poi_count, None)
                    .expect("valid count stat"),
            ],
            highlighted_poi_ids: vec![Uuid::new_v4()],
        })
        .expect("valid walk session")
    }
}
