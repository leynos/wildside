//! Builders for HTTP state ports and repository-backed service pairs.

mod user_state;

use std::sync::Arc;

use actix_web::web;

use backend::domain::ports::{
    CatalogueRepository, DescriptorRepository, EnrichmentProvenanceRepository,
    FixtureCatalogueRepository, FixtureDescriptorRepository, FixtureEnrichmentProvenanceRepository,
    FixtureOfflineBundleCommand, FixtureOfflineBundleQuery, FixtureWalkSessionCommand,
    FixtureWalkSessionQuery, OfflineBundleCommand, OfflineBundleQuery, PaginationErrorMetrics,
    RouteSubmissionService, WalkSessionCommand, WalkSessionQuery,
};
use backend::domain::{
    OfflineBundleCommandService, OfflineBundleQueryService, WalkSessionCommandService,
    WalkSessionQueryService,
};
use backend::inbound::http::state::{HttpState, HttpStateExtraPorts, HttpStatePorts};
use backend::outbound::persistence::DieselIdempotencyRepository;
use backend::outbound::persistence::{
    DieselCatalogueRepository, DieselDescriptorRepository, DieselEnrichmentProvenanceRepository,
    DieselOfflineBundleRepository, DieselWalkSessionRepository,
};

use self::user_state::{UserStatePortsBundle, compose_user_state_ports};
use super::ServerConfig;

/// Construct and return `(CatalogueRepository, DescriptorRepository)` by
/// selecting `DieselCatalogueRepository`/`DieselDescriptorRepository` when
/// `config.db_pool` is present, otherwise selecting
/// `FixtureCatalogueRepository`/`FixtureDescriptorRepository`.
fn build_catalogue_services(
    config: &ServerConfig,
) -> (Arc<dyn CatalogueRepository>, Arc<dyn DescriptorRepository>) {
    match &config.db_pool {
        Some(pool) => (
            Arc::new(DieselCatalogueRepository::new(pool.clone())),
            Arc::new(DieselDescriptorRepository::new(pool.clone())),
        ),
        None => (
            Arc::new(FixtureCatalogueRepository),
            Arc::new(FixtureDescriptorRepository),
        ),
    }
}

fn build_offline_bundles_pair(
    config: &ServerConfig,
) -> (Arc<dyn OfflineBundleCommand>, Arc<dyn OfflineBundleQuery>) {
    match &config.db_pool {
        Some(pool) => {
            let repo = Arc::new(DieselOfflineBundleRepository::new(pool.clone()));
            let idempotency_repo = Arc::new(DieselIdempotencyRepository::new(pool.clone()));
            (
                Arc::new(OfflineBundleCommandService::new(
                    repo.clone(),
                    idempotency_repo,
                    Arc::new(mockable::DefaultClock),
                )),
                Arc::new(OfflineBundleQueryService::new(repo)),
            )
        }
        None => (
            Arc::new(FixtureOfflineBundleCommand),
            Arc::new(FixtureOfflineBundleQuery),
        ),
    }
}

fn build_walk_sessions_pair(
    config: &ServerConfig,
) -> (Arc<dyn WalkSessionCommand>, Arc<dyn WalkSessionQuery>) {
    match &config.db_pool {
        Some(pool) => {
            let repo = Arc::new(DieselWalkSessionRepository::new(pool.clone()));
            (
                Arc::new(WalkSessionCommandService::new(repo.clone())),
                Arc::new(WalkSessionQueryService::new(repo)),
            )
        }
        None => (
            Arc::new(FixtureWalkSessionCommand),
            Arc::new(FixtureWalkSessionQuery),
        ),
    }
}

fn build_enrichment_provenance_repository(
    config: &ServerConfig,
) -> Arc<dyn EnrichmentProvenanceRepository> {
    match &config.db_pool {
        Some(pool) => Arc::new(DieselEnrichmentProvenanceRepository::new(pool.clone())),
        None => Arc::new(FixtureEnrichmentProvenanceRepository),
    }
}

/// Build HTTP state with the metrics recorder selected at server composition.
pub(crate) fn build_http_state_with_metrics(
    config: &ServerConfig,
    route_submission: Arc<dyn RouteSubmissionService>,
    pagination_error_metrics: Arc<dyn PaginationErrorMetrics>,
) -> web::Data<HttpState> {
    let UserStatePortsBundle {
        login,
        users,
        profile,
        interests,
        preferences,
        preferences_query,
        route_annotations,
        route_annotations_query,
    } = compose_user_state_ports(config, &pagination_error_metrics);
    let (offline_bundles, offline_bundles_query) = build_offline_bundles_pair(config);
    let (walk_sessions, walk_sessions_query) = build_walk_sessions_pair(config);
    let enrichment_provenance = build_enrichment_provenance_repository(config);
    let (catalogue, descriptors) = build_catalogue_services(config);

    web::Data::new(HttpState::new_with_extra_and_metrics(
        HttpStatePorts {
            login,
            users,
            profile,
            interests,
            preferences,
            preferences_query,
            route_annotations,
            route_annotations_query,
            route_submission,
            catalogue,
            descriptors,
        },
        HttpStateExtraPorts {
            offline_bundles,
            offline_bundles_query,
            enrichment_provenance,
            walk_sessions,
            walk_sessions_query,
            ..HttpStateExtraPorts::default()
        },
        pagination_error_metrics,
    ))
}
