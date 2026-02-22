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
