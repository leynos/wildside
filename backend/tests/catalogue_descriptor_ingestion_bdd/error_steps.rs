//! Failure-path BDD steps and helpers for ingestion repositories.

use std::future::Future;

use backend::domain::ports::{
    CatalogueIngestionRepository, CatalogueIngestionRepositoryError, DescriptorIngestionRepository,
    DescriptorIngestionRepositoryError,
};
use rstest_bdd_macros::{then, when};

use crate::snapshots::build_ingestion_snapshots;
use crate::{SharedContext, TestContext};

macro_rules! define_drop_table_upsert_step {
    (
        $fn_name:ident,
        $step_name:literal,
        $record_error_fn:ident,
        $repository_field:ident,
        $drop_sql:literal,
        $snapshot_field:ident,
        $upsert_method:ident
    ) => {
        #[when($step_name)]
        fn $fn_name(world: SharedContext) {
            $record_error_fn(
                &world,
                run_operation_with_dropped_table(
                    &world,
                    $drop_sql,
                    |ctx| ctx.$repository_field.clone(),
                    |repository| async move {
                        let value = build_ingestion_snapshots().$snapshot_field;
                        repository
                            .$upsert_method(std::slice::from_ref(&value))
                            .await
                    },
                ),
            );
        }
    };
}

define_drop_table_upsert_step!(
    the_tags_table_is_dropped_and_a_tag_upsert_is_attempted,
    "the tags table is dropped and a tag upsert is attempted",
    record_descriptor_error,
    descriptor_repository,
    "DROP TABLE tags;",
    tag,
    upsert_tags
);

#[then("the descriptor repository reports a query error")]
fn the_descriptor_repository_reports_a_query_error(world: SharedContext) {
    assert_world_query_error(
        &world,
        |ctx| &ctx.last_descriptor_error,
        |error| matches!(error, DescriptorIngestionRepositoryError::Query { .. }),
        "DescriptorIngestionRepositoryError::Query",
    );
}

define_drop_table_upsert_step!(
    the_route_categories_table_is_dropped_and_a_route_category_upsert_is_attempted,
    "the route categories table is dropped and a route category upsert is attempted",
    record_catalogue_error,
    catalogue_repository,
    "DROP TABLE route_categories CASCADE;",
    category,
    upsert_route_categories
);

#[then("the catalogue repository reports a query error")]
fn the_catalogue_repository_reports_a_query_error(world: SharedContext) {
    assert_world_query_error(
        &world,
        |ctx| &ctx.last_catalogue_error,
        |error| matches!(error, CatalogueIngestionRepositoryError::Query { .. }),
        "CatalogueIngestionRepositoryError::Query",
    );
}

fn record_descriptor_error(
    world: &SharedContext,
    error: Option<DescriptorIngestionRepositoryError>,
) {
    let mut ctx = world.lock().expect("context lock");
    ctx.last_descriptor_error = error;
}

fn record_catalogue_error(world: &SharedContext, error: Option<CatalogueIngestionRepositoryError>) {
    let mut ctx = world.lock().expect("context lock");
    ctx.last_catalogue_error = error;
}

fn assert_world_query_error<Error, SelectError, IsQuery>(
    world: &SharedContext,
    select_error: SelectError,
    is_query_error: IsQuery,
    expected_error_label: &str,
) where
    Error: std::fmt::Debug,
    SelectError: FnOnce(&TestContext) -> &Option<Error>,
    IsQuery: FnOnce(&Error) -> bool,
{
    let ctx = world.lock().expect("context lock");
    let error = select_error(&ctx);

    assert!(
        error.as_ref().is_some_and(is_query_error),
        "expected {expected_error_label}, got {:?}",
        error
    );
}

fn run_operation_with_dropped_table<Repo, Error, SelectRepository, Op, Fut>(
    world: &SharedContext,
    drop_table_sql: &str,
    select_repository: SelectRepository,
    operation: Op,
) -> Option<Error>
where
    SelectRepository: FnOnce(&TestContext) -> Repo,
    Op: FnOnce(Repo) -> Fut,
    Fut: Future<Output = Result<(), Error>>,
{
    let (repository, handle) = {
        let mut ctx = world.lock().expect("context lock");
        ctx.client
            .batch_execute(drop_table_sql)
            .expect("table should drop");
        (select_repository(&ctx), ctx.runtime.handle().clone())
    };

    let result = handle.block_on(operation(repository));
    result.err()
}
