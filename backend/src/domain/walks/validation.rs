//! Walk session validation and conversion helpers.

use std::collections::HashSet;

use uuid::Uuid;

use super::{
    WalkPrimaryStat, WalkPrimaryStatDraft, WalkSecondaryStat, WalkSecondaryStatDraft, WalkSession,
    WalkSessionDraft, WalkValidationError,
};

impl TryFrom<WalkPrimaryStatDraft> for WalkPrimaryStat {
    type Error = WalkValidationError;

    fn try_from(value: WalkPrimaryStatDraft) -> Result<Self, Self::Error> {
        Self::new(value.kind, value.value)
    }
}

impl TryFrom<WalkSecondaryStatDraft> for WalkSecondaryStat {
    type Error = WalkValidationError;

    fn try_from(value: WalkSecondaryStatDraft) -> Result<Self, Self::Error> {
        Self::new(value.kind, value.value, value.unit)
    }
}

impl TryFrom<WalkSessionDraft> for WalkSession {
    type Error = WalkValidationError;

    fn try_from(value: WalkSessionDraft) -> Result<Self, Self::Error> {
        if value
            .ended_at
            .is_some_and(|ended_at| ended_at < value.started_at)
        {
            return Err(WalkValidationError::EndedBeforeStarted);
        }

        validate_unique_primary_stat_kinds(value.primary_stats.as_slice())?;
        validate_unique_secondary_stat_kinds(value.secondary_stats.as_slice())?;
        validate_unique_poi_ids(value.highlighted_poi_ids.as_slice())?;

        Ok(Self {
            id: value.id,
            user_id: value.user_id,
            route_id: value.route_id,
            started_at: value.started_at,
            ended_at: value.ended_at,
            primary_stats: value.primary_stats,
            secondary_stats: value.secondary_stats,
            highlighted_poi_ids: value.highlighted_poi_ids,
        })
    }
}

fn validate_unique_primary_stat_kinds(
    stats: &[WalkPrimaryStat],
) -> Result<(), WalkValidationError> {
    let mut kinds = HashSet::new();
    for stat in stats {
        if !kinds.insert(stat.kind()) {
            return Err(WalkValidationError::DuplicatePrimaryStatKind { kind: stat.kind() });
        }
    }
    Ok(())
}

fn validate_unique_secondary_stat_kinds(
    stats: &[WalkSecondaryStat],
) -> Result<(), WalkValidationError> {
    let mut kinds = HashSet::new();
    for stat in stats {
        if !kinds.insert(stat.kind()) {
            return Err(WalkValidationError::DuplicateSecondaryStatKind { kind: stat.kind() });
        }
    }
    Ok(())
}

fn validate_unique_poi_ids(ids: &[Uuid]) -> Result<(), WalkValidationError> {
    let mut seen = HashSet::new();
    for poi_id in ids {
        if !seen.insert(*poi_id) {
            return Err(WalkValidationError::DuplicateHighlightedPoiId { poi_id: *poi_id });
        }
    }
    Ok(())
}
