//! Driving port for walk session mutations.
//!
//! This port records walk sessions and returns stable identifiers plus optional
//! completion projections.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{
    Error, WalkCompletionSummary, WalkPrimaryStat, WalkPrimaryStatDraft, WalkSecondaryStat,
    WalkSecondaryStatDraft, WalkSession, WalkSessionDraft, WalkValidationError,
};

/// Serializable walk session payload for driving ports.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalkSessionPayload {
    pub id: Uuid,
    pub user_id: crate::domain::UserId,
    pub route_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub primary_stats: Vec<WalkPrimaryStatDraft>,
    pub secondary_stats: Vec<WalkSecondaryStatDraft>,
    pub highlighted_poi_ids: Vec<Uuid>,
}

/// Serializable completion summary payload for driving ports.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalkCompletionSummaryPayload {
    pub session_id: Uuid,
    pub user_id: crate::domain::UserId,
    pub route_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub primary_stats: Vec<WalkPrimaryStatDraft>,
    pub secondary_stats: Vec<WalkSecondaryStatDraft>,
    pub highlighted_poi_ids: Vec<Uuid>,
}

impl TryFrom<WalkSessionPayload> for WalkSession {
    type Error = WalkValidationError;

    fn try_from(value: WalkSessionPayload) -> Result<Self, Self::Error> {
        let primary_stats = value
            .primary_stats
            .into_iter()
            .map(WalkPrimaryStat::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let secondary_stats = value
            .secondary_stats
            .into_iter()
            .map(WalkSecondaryStat::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        WalkSession::new(WalkSessionDraft {
            id: value.id,
            user_id: value.user_id,
            route_id: value.route_id,
            started_at: value.started_at,
            ended_at: value.ended_at,
            primary_stats,
            secondary_stats,
            highlighted_poi_ids: value.highlighted_poi_ids,
        })
    }
}

impl From<WalkSession> for WalkSessionPayload {
    fn from(value: WalkSession) -> Self {
        Self {
            id: value.id(),
            user_id: value.user_id().clone(),
            route_id: value.route_id(),
            started_at: value.started_at(),
            ended_at: value.ended_at(),
            primary_stats: value
                .primary_stats()
                .iter()
                .map(|stat| WalkPrimaryStatDraft {
                    kind: stat.kind(),
                    value: stat.value(),
                })
                .collect(),
            secondary_stats: value
                .secondary_stats()
                .iter()
                .map(|stat| WalkSecondaryStatDraft {
                    kind: stat.kind(),
                    value: stat.value(),
                    unit: stat.unit().map(str::to_owned),
                })
                .collect(),
            highlighted_poi_ids: value.highlighted_poi_ids().to_vec(),
        }
    }
}

impl From<WalkCompletionSummary> for WalkCompletionSummaryPayload {
    fn from(value: WalkCompletionSummary) -> Self {
        Self {
            session_id: value.session_id(),
            user_id: value.user_id().clone(),
            route_id: value.route_id(),
            started_at: value.started_at(),
            ended_at: value.ended_at(),
            primary_stats: value
                .primary_stats()
                .iter()
                .map(|stat| WalkPrimaryStatDraft {
                    kind: stat.kind(),
                    value: stat.value(),
                })
                .collect(),
            secondary_stats: value
                .secondary_stats()
                .iter()
                .map(|stat| WalkSecondaryStatDraft {
                    kind: stat.kind(),
                    value: stat.value(),
                    unit: stat.unit().map(str::to_owned),
                })
                .collect(),
            highlighted_poi_ids: value.highlighted_poi_ids().to_vec(),
        }
    }
}

/// Request to create a walk session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWalkSessionRequest {
    pub session: WalkSessionPayload,
}

/// Response from creating a walk session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWalkSessionResponse {
    pub session_id: Uuid,
    pub completion_summary: Option<WalkCompletionSummaryPayload>,
}

/// Driving port for walk session write operations.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait WalkSessionCommand: Send + Sync {
    /// Creates a walk session and returns its stable identifier plus optional
    /// completion summary when the payload includes an `ended_at` timestamp.
    ///
    /// Accepts `CreateWalkSessionRequest` and returns
    /// `CreateWalkSessionResponse`. Callers should handle `Result::Err(Error)`
    /// for validation and persistence failures at the boundary layer.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use chrono::{DateTime, Utc};
    /// # use uuid::Uuid;
    /// # use backend::domain::{UserId, WalkPrimaryStatDraft, WalkPrimaryStatKind, WalkSecondaryStatDraft, WalkSecondaryStatKind};
    /// # use backend::domain::ports::{CreateWalkSessionRequest, FixtureWalkSessionCommand, WalkSessionCommand, WalkSessionPayload};
    /// # async fn example() -> Result<(), backend::domain::Error> {
    /// let session_id = Uuid::new_v4();
    /// let command = backend::domain::ports::FixtureWalkSessionCommand;
    /// let request = backend::domain::ports::CreateWalkSessionRequest {
    ///     session: WalkSessionPayload {
    ///         id: session_id,
    ///         user_id: UserId::random(),
    ///         route_id: Uuid::new_v4(),
    ///         started_at: DateTime::parse_from_rfc3339("2026-01-02T03:04:05Z")
    ///             .expect("valid timestamp")
    ///             .with_timezone(&Utc),
    ///         ended_at: None,
    ///         primary_stats: vec![WalkPrimaryStatDraft {
    ///             kind: WalkPrimaryStatKind::Distance,
    ///             value: 1_000.0,
    ///         }],
    ///         secondary_stats: vec![WalkSecondaryStatDraft {
    ///             kind: WalkSecondaryStatKind::Energy,
    ///             value: 120.0,
    ///             unit: Some("kcal".to_owned()),
    ///         }],
    ///         highlighted_poi_ids: vec![Uuid::new_v4()],
    ///     },
    /// };
    /// let response = command.create_session(request).await?;
    /// assert_eq!(response.session_id, session_id);
    /// # Ok(())
    /// # }
    /// ```
    async fn create_session(
        &self,
        request: CreateWalkSessionRequest,
    ) -> Result<CreateWalkSessionResponse, Error>;
}

/// Fixture command implementation for tests that do not need persistence.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureWalkSessionCommand;

#[async_trait]
impl WalkSessionCommand for FixtureWalkSessionCommand {
    async fn create_session(
        &self,
        request: CreateWalkSessionRequest,
    ) -> Result<CreateWalkSessionResponse, Error> {
        let session = WalkSession::try_from(request.session).map_err(|err| {
            Error::invalid_request(format!("invalid walk session payload: {err}"))
        })?;

        Ok(CreateWalkSessionResponse {
            session_id: session.id(),
            completion_summary: session.completion_summary().ok().map(Into::into),
        })
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.

    use chrono::{DateTime, Utc};
    use rstest::{fixture, rstest};

    use super::*;
    use crate::domain::{WalkPrimaryStatKind, WalkSecondaryStatKind};

    fn fixture_timestamp() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-01-02T03:04:05Z")
            .expect("RFC3339 fixture timestamp")
            .with_timezone(&Utc)
    }

    #[fixture]
    fn sample_payload() -> WalkSessionPayload {
        let started_at = fixture_timestamp();
        WalkSessionPayload {
            id: Uuid::new_v4(),
            user_id: crate::domain::UserId::random(),
            route_id: Uuid::new_v4(),
            started_at,
            ended_at: Some(started_at),
            primary_stats: vec![WalkPrimaryStatDraft {
                kind: WalkPrimaryStatKind::Distance,
                value: 1000.0,
            }],
            secondary_stats: vec![WalkSecondaryStatDraft {
                kind: WalkSecondaryStatKind::Energy,
                value: 120.0,
                unit: Some("kcal".to_owned()),
            }],
            highlighted_poi_ids: vec![Uuid::new_v4()],
        }
    }

    #[rstest]
    #[tokio::test]
    async fn fixture_command_preserves_session_id(sample_payload: WalkSessionPayload) {
        let command = FixtureWalkSessionCommand;
        let request = CreateWalkSessionRequest {
            session: sample_payload,
        };

        let response = command
            .create_session(request.clone())
            .await
            .expect("fixture create succeeds");

        assert_eq!(response.session_id, request.session.id);
        assert!(response.completion_summary.is_some());
    }

    #[rstest]
    #[tokio::test]
    async fn payload_round_trip_through_domain_entity(sample_payload: WalkSessionPayload) {
        let payload = sample_payload;

        let session = WalkSession::try_from(payload.clone()).expect("valid session payload");
        let restored = WalkSessionPayload::from(session);

        assert_eq!(restored.id, payload.id);
        assert_eq!(restored.route_id, payload.route_id);
        assert_eq!(restored.primary_stats.len(), payload.primary_stats.len());
    }
}
