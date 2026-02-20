//! Port for walk session persistence and completion summary reads.

use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::{UserId, WalkCompletionSummary, WalkSession};

use super::define_port_error;

define_port_error! {
    /// Errors raised by walk session repository adapters.
    pub enum WalkSessionRepositoryError {
        /// Repository connection could not be established.
        Connection { message: String } =>
            "walk session repository connection failed: {message}",
        /// Query or mutation failed during execution.
        Query { message: String } =>
            "walk session repository query failed: {message}",
    }
}

/// Port for writing walk sessions and reading completion summaries.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait WalkSessionRepository: Send + Sync {
    /// Persist a walk session.
    async fn save(&self, session: &WalkSession) -> Result<(), WalkSessionRepositoryError>;

    /// Find a walk session by id.
    async fn find_by_id(
        &self,
        session_id: &Uuid,
    ) -> Result<Option<WalkSession>, WalkSessionRepositoryError>;

    /// Read completion summaries for a user.
    async fn list_completion_summaries_for_user(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<WalkCompletionSummary>, WalkSessionRepositoryError>;
}

/// Fixture implementation for tests that do not exercise walk persistence.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureWalkSessionRepository;

#[async_trait]
impl WalkSessionRepository for FixtureWalkSessionRepository {
    async fn save(&self, _session: &WalkSession) -> Result<(), WalkSessionRepositoryError> {
        Ok(())
    }

    async fn find_by_id(
        &self,
        _session_id: &Uuid,
    ) -> Result<Option<WalkSession>, WalkSessionRepositoryError> {
        Ok(None)
    }

    async fn list_completion_summaries_for_user(
        &self,
        _user_id: &UserId,
    ) -> Result<Vec<WalkCompletionSummary>, WalkSessionRepositoryError> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.

    use chrono::Utc;
    use rstest::rstest;

    use super::*;
    use crate::domain::{
        WalkPrimaryStat, WalkPrimaryStatKind, WalkSecondaryStat, WalkSecondaryStatKind,
        WalkSessionDraft,
    };

    fn build_session(user_id: UserId) -> WalkSession {
        let started_at = Utc::now();
        WalkSession::new(WalkSessionDraft {
            id: Uuid::new_v4(),
            user_id,
            route_id: Uuid::new_v4(),
            started_at,
            ended_at: Some(started_at),
            primary_stats: vec![
                WalkPrimaryStat::new(WalkPrimaryStatKind::Distance, 1000.0)
                    .expect("valid primary stat"),
            ],
            secondary_stats: vec![
                WalkSecondaryStat::new(
                    WalkSecondaryStatKind::Energy,
                    100.0,
                    Some("kcal".to_owned()),
                )
                .expect("valid secondary stat"),
            ],
            highlighted_poi_ids: vec![Uuid::new_v4()],
        })
        .expect("valid session")
    }

    #[rstest]
    #[tokio::test]
    async fn fixture_find_returns_none() {
        let repo = FixtureWalkSessionRepository;
        let found = repo
            .find_by_id(&Uuid::new_v4())
            .await
            .expect("fixture lookup succeeds");
        assert!(found.is_none());
    }

    #[rstest]
    #[tokio::test]
    async fn fixture_list_returns_empty() {
        let repo = FixtureWalkSessionRepository;
        let listed = repo
            .list_completion_summaries_for_user(&UserId::random())
            .await
            .expect("fixture list succeeds");
        assert!(listed.is_empty());
    }

    #[rstest]
    #[tokio::test]
    async fn fixture_save_succeeds() {
        let repo = FixtureWalkSessionRepository;
        let session = build_session(UserId::random());

        repo.save(&session).await.expect("fixture save succeeds");
    }

    #[rstest]
    fn query_error_formats_message() {
        let err = WalkSessionRepositoryError::query("broken sql");
        let msg = err.to_string();
        assert!(msg.contains("broken sql"));
    }
}
