//! Walk-session BDD step definitions.

use backend::domain::ports::{WalkSessionRepository, WalkSessionRepositoryError};
use futures::executor::block_on;
use rstest_bdd_macros::{then, when};

use super::contract_checks::assert_walk_lookup_and_summary_filtering_contract;
use super::steps_helpers::{SharedContext, execute_drop_table_save_scenario};

#[when("a completed walk session is saved and queried")]
fn a_completed_walk_session_is_saved_and_queried(world: SharedContext) {
    let (walk_repo, walk_session) = {
        let ctx = world.lock().expect("context lock");
        (ctx.walk_repo.clone(), ctx.walk_session.clone())
    };

    let (save_result, find_result, summaries_result) = block_on(async {
        let save_result = walk_repo.save(&walk_session).await;
        if save_result.is_err() {
            return (save_result, Ok(None), Ok(Vec::new()));
        }

        let find_result = walk_repo.find_by_id(&walk_session.id()).await;
        if find_result.is_err() {
            return (save_result, find_result, Ok(Vec::new()));
        }

        let summaries_result = walk_repo
            .list_completion_summaries_for_user(walk_session.user_id())
            .await;
        (save_result, find_result, summaries_result)
    });

    let mut ctx = world.lock().expect("context lock");
    if let Err(err) = save_result {
        ctx.last_walk_error = Some(err);
        return;
    }

    match find_result {
        Ok(found) => ctx.last_found_session = Some(found),
        Err(err) => {
            ctx.last_walk_error = Some(err);
            return;
        }
    }

    match summaries_result {
        Ok(summaries) => {
            ctx.last_walk_summaries = Some(summaries);
            ctx.last_walk_error = None;
        }
        Err(err) => ctx.last_walk_error = Some(err),
    }
}

#[then("the walk session and completion summary are returned")]
fn the_walk_session_and_completion_summary_are_returned(world: SharedContext) {
    let ctx = world.lock().expect("context lock");
    assert!(ctx.last_walk_error.is_none(), "{:?}", ctx.last_walk_error);

    let found = ctx
        .last_found_session
        .as_ref()
        .expect("find should execute")
        .as_ref()
        .expect("session should exist");
    assert_eq!(found.id(), ctx.walk_session.id());

    let summaries = ctx
        .last_walk_summaries
        .as_ref()
        .expect("summary list should execute");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].session_id(), ctx.walk_session.id());
}

#[when("walk missing lookup and completion summary filtering contracts are exercised")]
fn walk_missing_lookup_and_completion_summary_filtering_contracts_are_exercised(
    world: SharedContext,
) {
    let (walk_repo, walk_session) = {
        let ctx = world.lock().expect("context lock");
        (ctx.walk_repo.clone(), ctx.walk_session.clone())
    };
    assert_walk_lookup_and_summary_filtering_contract(walk_repo, walk_session);
}

#[when("the walk session table is dropped and a walk save is attempted")]
fn the_walk_session_table_is_dropped_and_a_walk_save_is_attempted(world: SharedContext) {
    execute_drop_table_save_scenario(
        world,
        "walk_sessions",
        |ctx| {
            (
                ctx.database_url.clone(),
                ctx.walk_repo.clone(),
                ctx.walk_session.clone(),
            )
        },
        |walk_repo, walk_session| block_on(async { walk_repo.save(&walk_session).await }),
        |ctx, error| ctx.last_walk_error = error,
    );
}

#[then("the walk session repository reports a query error")]
fn the_walk_session_repository_reports_a_query_error(world: SharedContext) {
    let ctx = world.lock().expect("context lock");
    assert!(matches!(
        ctx.last_walk_error,
        Some(WalkSessionRepositoryError::Query { .. })
    ));
}
