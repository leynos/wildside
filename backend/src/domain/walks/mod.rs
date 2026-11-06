//! Walk session and completion summary domain types.
//!
//! Walk sessions capture completion-relevant statistics for route walks.
//! Completion summaries are derived from finished sessions and serve the
//! repository read model used by outbound adapters.

use std::fmt;

use uuid::Uuid;

mod session;
mod stats;
#[cfg(test)]
mod tests;
mod validation;

pub use session::{WalkCompletionSummary, WalkSession, WalkSessionDraft};
pub use stats::{
    ParseWalkPrimaryStatKindError, ParseWalkSecondaryStatKindError, WalkPrimaryStat,
    WalkPrimaryStatDraft, WalkPrimaryStatKind, WalkSecondaryStat, WalkSecondaryStatDraft,
    WalkSecondaryStatKind,
};

/// Validation errors raised by walk session constructors.
#[derive(Debug, Clone, PartialEq)]
pub enum WalkValidationError {
    /// A primary stat value was non-finite or negative.
    NegativePrimaryStatValue {
        /// Statistic whose value was rejected.
        kind: WalkPrimaryStatKind,
        /// The rejected value.
        value: f64,
    },
    /// A secondary stat value was non-finite or negative.
    NegativeSecondaryStatValue {
        /// Statistic whose value was rejected.
        kind: WalkSecondaryStatKind,
        /// The rejected value.
        value: f64,
    },
    /// A secondary stat unit label was blank after trimming.
    EmptySecondaryStatUnit,
    /// The session end time precedes its start time.
    EndedBeforeStarted,
    /// The same primary stat kind was supplied more than once.
    DuplicatePrimaryStatKind {
        /// The repeated statistic kind.
        kind: WalkPrimaryStatKind,
    },
    /// The same secondary stat kind was supplied more than once.
    DuplicateSecondaryStatKind {
        /// The repeated statistic kind.
        kind: WalkSecondaryStatKind,
    },
    /// The same POI was highlighted more than once in a session.
    DuplicateHighlightedPoiId {
        /// The repeated POI identifier.
        poi_id: Uuid,
    },
    /// A completion summary was requested for a session without an end time.
    SessionNotCompleted,
}

impl fmt::Display for WalkValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NegativePrimaryStatValue { kind, value } => write!(
                f,
                "walk primary stat {kind} must be finite and non-negative (got {value})"
            ),
            Self::NegativeSecondaryStatValue { kind, value } => write!(
                f,
                "walk secondary stat {kind} must be finite and non-negative (got {value})"
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
