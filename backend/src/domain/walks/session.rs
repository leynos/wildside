//! Walk session entities and completion summaries.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::UserId;

use super::{WalkPrimaryStat, WalkSecondaryStat, WalkValidationError};

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
///
/// # Examples
///
/// ```rust,ignore
/// # let draft = sample_walk_session_draft()?;
/// let session = backend::domain::WalkSession::new(draft)?;
/// assert!(!session.primary_stats().is_empty());
/// Ok::<(), backend::domain::WalkValidationError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct WalkSession {
    pub(super) id: Uuid,
    pub(super) user_id: UserId,
    pub(super) route_id: Uuid,
    pub(super) started_at: DateTime<Utc>,
    pub(super) ended_at: Option<DateTime<Utc>>,
    pub(super) primary_stats: Vec<WalkPrimaryStat>,
    pub(super) secondary_stats: Vec<WalkSecondaryStat>,
    pub(super) highlighted_poi_ids: Vec<Uuid>,
}

impl WalkSession {
    /// Creates a validated walk session.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let draft = sample_walk_session_draft()?;
    /// let session = backend::domain::WalkSession::new(draft)?;
    /// assert!(session.ended_at().is_some());
    /// Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn new(draft: WalkSessionDraft) -> Result<Self, WalkValidationError> {
        Self::try_from(draft)
    }

    /// Returns the session id.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let session = sample_walk_session()?;
    /// assert!(!session.id().is_nil());
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Returns the owning user id.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let session = sample_walk_session()?;
    /// let _ = session.user_id();
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    /// Returns the route id.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let session = sample_walk_session()?;
    /// assert!(!session.route_id().is_nil());
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn route_id(&self) -> Uuid {
        self.route_id
    }

    /// Returns the start timestamp.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let session = sample_walk_session()?;
    /// let _ = session.started_at();
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn started_at(&self) -> DateTime<Utc> {
        self.started_at
    }

    /// Returns the optional end timestamp.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let session = sample_walk_session()?;
    /// let _ = session.ended_at();
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn ended_at(&self) -> Option<DateTime<Utc>> {
        self.ended_at
    }

    /// Returns primary stats in submission order.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let session = sample_walk_session()?;
    /// assert!(!session.primary_stats().is_empty());
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn primary_stats(&self) -> &[WalkPrimaryStat] {
        self.primary_stats.as_slice()
    }

    /// Returns secondary stats in submission order.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let session = sample_walk_session()?;
    /// assert!(!session.secondary_stats().is_empty());
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn secondary_stats(&self) -> &[WalkSecondaryStat] {
        self.secondary_stats.as_slice()
    }

    /// Returns highlighted point-of-interest ids.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let session = sample_walk_session()?;
    /// assert!(!session.highlighted_poi_ids().is_empty());
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn highlighted_poi_ids(&self) -> &[Uuid] {
        self.highlighted_poi_ids.as_slice()
    }

    /// Derives a completion summary from a completed session.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let session = sample_walk_session()?;
    /// let summary = session.completion_summary()?;
    /// assert_eq!(summary.session_id(), session.id());
    /// Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
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

/// Completion summary derived from a completed walk session.
///
/// # Examples
///
/// ```rust,ignore
/// # let session = sample_walk_session()?;
/// let summary = session.completion_summary()?;
/// assert_eq!(summary.route_id(), session.route_id());
/// Ok::<(), backend::domain::WalkValidationError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct WalkCompletionSummary {
    pub(super) session_id: Uuid,
    pub(super) user_id: UserId,
    pub(super) route_id: Uuid,
    pub(super) started_at: DateTime<Utc>,
    pub(super) ended_at: DateTime<Utc>,
    pub(super) primary_stats: Vec<WalkPrimaryStat>,
    pub(super) secondary_stats: Vec<WalkSecondaryStat>,
    pub(super) highlighted_poi_ids: Vec<Uuid>,
}

impl WalkCompletionSummary {
    /// Returns the originating walk session id.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let summary = sample_walk_completion_summary()?;
    /// assert!(!summary.session_id().is_nil());
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    /// Returns the owning user id.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let summary = sample_walk_completion_summary()?;
    /// let _ = summary.user_id();
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    /// Returns the route id.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let summary = sample_walk_completion_summary()?;
    /// assert!(!summary.route_id().is_nil());
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn route_id(&self) -> Uuid {
        self.route_id
    }

    /// Returns the walk start timestamp.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let summary = sample_walk_completion_summary()?;
    /// let _ = summary.started_at();
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn started_at(&self) -> DateTime<Utc> {
        self.started_at
    }

    /// Returns the walk end timestamp.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let summary = sample_walk_completion_summary()?;
    /// assert!(summary.ended_at() >= summary.started_at());
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn ended_at(&self) -> DateTime<Utc> {
        self.ended_at
    }

    /// Returns primary summary stats.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let summary = sample_walk_completion_summary()?;
    /// assert!(!summary.primary_stats().is_empty());
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn primary_stats(&self) -> &[WalkPrimaryStat] {
        self.primary_stats.as_slice()
    }

    /// Returns secondary summary stats.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let summary = sample_walk_completion_summary()?;
    /// assert!(!summary.secondary_stats().is_empty());
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn secondary_stats(&self) -> &[WalkSecondaryStat] {
        self.secondary_stats.as_slice()
    }

    /// Returns highlighted point-of-interest ids.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let summary = sample_walk_completion_summary()?;
    /// let _ = summary.highlighted_poi_ids();
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn highlighted_poi_ids(&self) -> &[Uuid] {
        self.highlighted_poi_ids.as_slice()
    }
}
