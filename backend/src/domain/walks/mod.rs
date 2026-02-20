//! Walk session and completion summary domain types.
//!
//! Walk sessions capture completion-relevant statistics for route walks.
//! Completion summaries are derived from finished sessions and serve the
//! repository read model used by outbound adapters.

use std::collections::HashSet;
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::UserId;

#[cfg(test)]
mod tests;

/// Validation errors raised by walk session constructors.
#[derive(Debug, Clone, PartialEq)]
pub enum WalkValidationError {
    NegativePrimaryStatValue {
        kind: WalkPrimaryStatKind,
        value: f64,
    },
    NegativeSecondaryStatValue {
        kind: WalkSecondaryStatKind,
        value: f64,
    },
    EmptySecondaryStatUnit,
    EndedBeforeStarted,
    DuplicatePrimaryStatKind {
        kind: WalkPrimaryStatKind,
    },
    DuplicateSecondaryStatKind {
        kind: WalkSecondaryStatKind,
    },
    DuplicateHighlightedPoiId {
        poi_id: Uuid,
    },
    SessionNotCompleted,
}

impl fmt::Display for WalkValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NegativePrimaryStatValue { kind, value } => write!(
                f,
                "walk primary stat {kind} must be non-negative (got {value})"
            ),
            Self::NegativeSecondaryStatValue { kind, value } => write!(
                f,
                "walk secondary stat {kind} must be non-negative (got {value})"
            ),
            Self::EmptySecondaryStatUnit => {
                write!(f, "walk secondary stat unit must not be blank")
            }
            Self::EndedBeforeStarted => {
                write!(f, "walk session ended_at must be >= started_at")
            }
            Self::DuplicatePrimaryStatKind { kind } => {
                write!(f, "walk session has duplicate primary stat kind {kind}")
            }
            Self::DuplicateSecondaryStatKind { kind } => {
                write!(f, "walk session has duplicate secondary stat kind {kind}")
            }
            Self::DuplicateHighlightedPoiId { poi_id } => {
                write!(f, "walk session has duplicate highlighted poi id {poi_id}")
            }
            Self::SessionNotCompleted => {
                write!(f, "walk completion summary requires ended_at")
            }
        }
    }
}

impl std::error::Error for WalkValidationError {}

/// Primary walk-stat categories surfaced by the PWA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalkPrimaryStatKind {
    Distance,
    Duration,
}

impl fmt::Display for WalkPrimaryStatKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Distance => f.write_str("distance"),
            Self::Duration => f.write_str("duration"),
        }
    }
}

/// Secondary walk-stat categories surfaced by the PWA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalkSecondaryStatKind {
    Energy,
    Count,
}

impl fmt::Display for WalkSecondaryStatKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Energy => f.write_str("energy"),
            Self::Count => f.write_str("count"),
        }
    }
}

/// A primary completion statistic for a walk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "WalkPrimaryStatDraft")]
pub struct WalkPrimaryStat {
    kind: WalkPrimaryStatKind,
    value: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalkPrimaryStatDraft {
    pub kind: WalkPrimaryStatKind,
    pub value: f64,
}

impl WalkPrimaryStat {
    pub fn new(kind: WalkPrimaryStatKind, value: f64) -> Result<Self, WalkValidationError> {
        if value < 0.0 {
            return Err(WalkValidationError::NegativePrimaryStatValue { kind, value });
        }
        Ok(Self { kind, value })
    }

    pub fn kind(&self) -> WalkPrimaryStatKind {
        self.kind
    }

    pub fn value(&self) -> f64 {
        self.value
    }
}

impl TryFrom<WalkPrimaryStatDraft> for WalkPrimaryStat {
    type Error = WalkValidationError;

    fn try_from(value: WalkPrimaryStatDraft) -> Result<Self, Self::Error> {
        Self::new(value.kind, value.value)
    }
}

/// A secondary completion statistic for a walk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "WalkSecondaryStatDraft")]
pub struct WalkSecondaryStat {
    kind: WalkSecondaryStatKind,
    value: f64,
    unit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalkSecondaryStatDraft {
    pub kind: WalkSecondaryStatKind,
    pub value: f64,
    pub unit: Option<String>,
}

impl WalkSecondaryStat {
    pub fn new(
        kind: WalkSecondaryStatKind,
        value: f64,
        unit: Option<String>,
    ) -> Result<Self, WalkValidationError> {
        if value < 0.0 {
            return Err(WalkValidationError::NegativeSecondaryStatValue { kind, value });
        }

        if unit.as_deref().map(str::trim).is_some_and(str::is_empty) {
            return Err(WalkValidationError::EmptySecondaryStatUnit);
        }

        Ok(Self {
            kind,
            value,
            unit: unit.map(|v| v.trim().to_owned()),
        })
    }

    pub fn kind(&self) -> WalkSecondaryStatKind {
        self.kind
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn unit(&self) -> Option<&str> {
        self.unit.as_deref()
    }
}

impl TryFrom<WalkSecondaryStatDraft> for WalkSecondaryStat {
    type Error = WalkValidationError;

    fn try_from(value: WalkSecondaryStatDraft) -> Result<Self, Self::Error> {
        Self::new(value.kind, value.value, value.unit)
    }
}

/// Input payload for [`WalkSession::new`].
#[derive(Debug, Clone)]
pub struct WalkSessionDraft {
    pub id: Uuid,
    pub user_id: UserId,
    pub route_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub primary_stats: Vec<WalkPrimaryStat>,
    pub secondary_stats: Vec<WalkSecondaryStat>,
    pub highlighted_poi_ids: Vec<Uuid>,
}

/// A persisted walk session with completion-related payloads.
#[derive(Debug, Clone, PartialEq)]
pub struct WalkSession {
    id: Uuid,
    user_id: UserId,
    route_id: Uuid,
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
    primary_stats: Vec<WalkPrimaryStat>,
    secondary_stats: Vec<WalkSecondaryStat>,
    highlighted_poi_ids: Vec<Uuid>,
}

impl WalkSession {
    pub fn new(draft: WalkSessionDraft) -> Result<Self, WalkValidationError> {
        Self::try_from(draft)
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn route_id(&self) -> Uuid {
        self.route_id
    }

    pub fn started_at(&self) -> DateTime<Utc> {
        self.started_at
    }

    pub fn ended_at(&self) -> Option<DateTime<Utc>> {
        self.ended_at
    }

    pub fn primary_stats(&self) -> &[WalkPrimaryStat] {
        self.primary_stats.as_slice()
    }

    pub fn secondary_stats(&self) -> &[WalkSecondaryStat] {
        self.secondary_stats.as_slice()
    }

    pub fn highlighted_poi_ids(&self) -> &[Uuid] {
        self.highlighted_poi_ids.as_slice()
    }

    pub fn completion_summary(&self) -> Result<WalkCompletionSummary, WalkValidationError> {
        let ended_at = self
            .ended_at
            .ok_or(WalkValidationError::SessionNotCompleted)?;
        Ok(WalkCompletionSummary {
            session_id: self.id,
            user_id: self.user_id.clone(),
            route_id: self.route_id,
            started_at: self.started_at,
            ended_at,
            primary_stats: self.primary_stats.clone(),
            secondary_stats: self.secondary_stats.clone(),
            highlighted_poi_ids: self.highlighted_poi_ids.clone(),
        })
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

/// Completion summary derived from a completed walk session.
#[derive(Debug, Clone, PartialEq)]
pub struct WalkCompletionSummary {
    session_id: Uuid,
    user_id: UserId,
    route_id: Uuid,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
    primary_stats: Vec<WalkPrimaryStat>,
    secondary_stats: Vec<WalkSecondaryStat>,
    highlighted_poi_ids: Vec<Uuid>,
}

impl WalkCompletionSummary {
    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn route_id(&self) -> Uuid {
        self.route_id
    }

    pub fn started_at(&self) -> DateTime<Utc> {
        self.started_at
    }

    pub fn ended_at(&self) -> DateTime<Utc> {
        self.ended_at
    }

    pub fn primary_stats(&self) -> &[WalkPrimaryStat] {
        self.primary_stats.as_slice()
    }

    pub fn secondary_stats(&self) -> &[WalkSecondaryStat] {
        self.secondary_stats.as_slice()
    }

    pub fn highlighted_poi_ids(&self) -> &[Uuid] {
        self.highlighted_poi_ids.as_slice()
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
