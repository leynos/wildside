//! Contract-check helpers for offline bundle and walk session BDD steps.

use backend::domain::ports::{
    OfflineBundleRepository, OfflineBundleRepositoryError, WalkSessionRepository,
    WalkSessionRepositoryError,
};
use backend::domain::{OfflineBundle, WalkSession, WalkSessionDraft};
use chrono::Duration;
use futures::executor::block_on;

use super::repository_impl::{PgOfflineBundleRepository, PgWalkSessionRepository};

/// Assert delete and lookup contracts for offline bundles.
pub fn assert_offline_delete_and_lookup_contract(
    offline_repo: PgOfflineBundleRepository,
    route_bundle: OfflineBundle,
) {
    let missing_bundle_id = uuid::Uuid::new_v4();

    let (was_deleted, deleted_lookup, missing_lookup) = block_on(async {
        offline_repo.save(&route_bundle).await?;
        let was_deleted = offline_repo.delete(&route_bundle.id()).await?;
        let deleted_lookup = offline_repo.find_by_id(&route_bundle.id()).await?;
        let missing_lookup = offline_repo.find_by_id(&missing_bundle_id).await?;
        Ok::<_, OfflineBundleRepositoryError>((was_deleted, deleted_lookup, missing_lookup))
    })
    .expect("offline delete/find contract checks should succeed");

    assert!(was_deleted, "expected delete to report a removed row");
    assert!(
        deleted_lookup.is_none(),
        "expected deleted bundle lookup to be None"
    );
    assert!(
        missing_lookup.is_none(),
        "expected random-id lookup to be None"
    );
}

/// Assert walk lookup and summary filtering contracts for walk sessions.
pub fn assert_walk_lookup_and_summary_filtering_contract(
    walk_repo: PgWalkSessionRepository,
    walk_session: WalkSession,
) {
    let missing_session_id = uuid::Uuid::new_v4();
    let earlier_completed = WalkSession::new(WalkSessionDraft {
        id: uuid::Uuid::new_v4(),
        user_id: walk_session.user_id().clone(),
        route_id: walk_session.route_id(),
        started_at: walk_session.started_at(),
        ended_at: Some(
            walk_session
                .ended_at()
                .expect("completed walk session should have ended_at")
                - Duration::minutes(10),
        ),
        primary_stats: walk_session.primary_stats().to_vec(),
        secondary_stats: walk_session.secondary_stats().to_vec(),
        highlighted_poi_ids: walk_session.highlighted_poi_ids().to_vec(),
    })
    .expect("valid earlier completed session");
    let incomplete_session = WalkSession::new(WalkSessionDraft {
        id: uuid::Uuid::new_v4(),
        user_id: walk_session.user_id().clone(),
        route_id: walk_session.route_id(),
        started_at: walk_session.started_at(),
        ended_at: None,
        primary_stats: walk_session.primary_stats().to_vec(),
        secondary_stats: walk_session.secondary_stats().to_vec(),
        highlighted_poi_ids: walk_session.highlighted_poi_ids().to_vec(),
    })
    .expect("valid incomplete session");

    let (missing_lookup, summaries) = block_on(async {
        walk_repo.save(&walk_session).await?;
        walk_repo.save(&earlier_completed).await?;
        walk_repo.save(&incomplete_session).await?;
        let missing_lookup = walk_repo.find_by_id(&missing_session_id).await?;
        let summaries = walk_repo
            .list_completion_summaries_for_user(walk_session.user_id())
            .await?;
        Ok::<_, WalkSessionRepositoryError>((missing_lookup, summaries))
    })
    .expect("walk repository edge-contract checks should succeed");

    assert!(
        missing_lookup.is_none(),
        "expected missing-id walk lookup to be None"
    );
    assert_eq!(
        summaries.len(),
        2,
        "expected only completed sessions in summaries"
    );
    assert_eq!(
        summaries[0].session_id(),
        walk_session.id(),
        "expected most recent completed session first"
    );
    assert_eq!(
        summaries[1].session_id(),
        earlier_completed.id(),
        "expected earlier completed session second"
    );
}
