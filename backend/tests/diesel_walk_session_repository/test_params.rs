//! Builder-style test parameters for walk-session integration fixtures.

use backend::domain::{
    UserId, WalkPrimaryStat, WalkPrimaryStatKind, WalkSecondaryStat, WalkSecondaryStatKind,
    WalkSession, WalkSessionDraft,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

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

    pub(crate) fn with_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    pub(crate) fn with_ended_at(mut self, ended_at: DateTime<Utc>) -> Self {
        self.ended_at = Some(ended_at);
        self
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "test builder intentionally exposes explicit stat values"
    )]
    pub(crate) fn with_stats(
        mut self,
        distance: f64,
        duration: f64,
        energy: f64,
        poi_count: f64,
    ) -> Self {
        self.distance = distance;
        self.duration = duration;
        self.energy = energy;
        self.poi_count = poi_count;
        self
    }

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
