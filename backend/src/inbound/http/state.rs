//! Shared HTTP adapter state.
//!
//! HTTP handlers accept this state via `actix_web::web::Data` so they only
//! depend on domain ports (use-cases) and remain testable without I/O.

use std::sync::Arc;

use crate::domain::ports::{
    CatalogueRepository, DescriptorRepository, LoginService, NoOpPaginationErrorMetrics,
    PaginationErrorMetrics, RouteAnnotationsCommand, RouteAnnotationsQuery, RouteSubmissionService,
    UserInterestsCommand, UserPreferencesCommand, UserPreferencesQuery, UserProfileQuery,
    UsersQuery,
};
use crate::domain::ports::{
    EnrichmentProvenanceRepository, FixtureEnrichmentProvenanceRepository,
    FixtureOfflineBundleCommand, FixtureOfflineBundleQuery, FixtureWalkSessionCommand,
    FixtureWalkSessionQuery, OfflineBundleCommand, OfflineBundleQuery, WalkSessionCommand,
    WalkSessionQuery,
};

/// Parameter object bundling all port implementations for HTTP handlers.
#[derive(Clone)]
pub struct HttpStatePorts {
    /// Authentication use case for login flows.
    pub login: Arc<dyn LoginService>,
    /// Use case for listing users.
    pub users: Arc<dyn UsersQuery>,
    /// Use case for reading the current user's profile.
    pub profile: Arc<dyn UserProfileQuery>,
    /// Use case for updating a user's interest theme selections.
    pub interests: Arc<dyn UserInterestsCommand>,
    /// Use case for updating user preferences, guarded by revision checks.
    pub preferences: Arc<dyn UserPreferencesCommand>,
    /// Use case for fetching user preferences.
    pub preferences_query: Arc<dyn UserPreferencesQuery>,
    /// Use case for updating route annotations, guarded by revision checks.
    pub route_annotations: Arc<dyn RouteAnnotationsCommand>,
    /// Use case for fetching route annotations.
    pub route_annotations_query: Arc<dyn RouteAnnotationsQuery>,
    /// Use case for submitting route requests with idempotency support.
    pub route_submission: Arc<dyn RouteSubmissionService>,
    /// Port for reading Explore catalogue snapshots.
    pub catalogue: Arc<dyn CatalogueRepository>,
    /// Port for reading descriptor registries.
    pub descriptors: Arc<dyn DescriptorRepository>,
}

/// Optional ports for endpoints introduced after the initial HTTP state shape.
#[derive(Clone)]
pub struct HttpStateExtraPorts {
    /// Driving port for offline bundle mutations.
    pub offline_bundles: Arc<dyn OfflineBundleCommand>,
    /// Driving port for offline bundle reads.
    pub offline_bundles_query: Arc<dyn OfflineBundleQuery>,
    /// Port for persisting and reporting enrichment provenance.
    pub enrichment_provenance: Arc<dyn EnrichmentProvenanceRepository>,
    /// Driving port for walk session writes.
    pub walk_sessions: Arc<dyn WalkSessionCommand>,
    /// Driving port for walk session reads.
    pub walk_sessions_query: Arc<dyn WalkSessionQuery>,
}

impl Default for HttpStateExtraPorts {
    fn default() -> Self {
        Self {
            offline_bundles: Arc::new(FixtureOfflineBundleCommand),
            offline_bundles_query: Arc::new(FixtureOfflineBundleQuery),
            enrichment_provenance: Arc::new(FixtureEnrichmentProvenanceRepository),
            walk_sessions: Arc::new(FixtureWalkSessionCommand),
            walk_sessions_query: Arc::new(FixtureWalkSessionQuery),
        }
    }
}

/// Dependency bundle for HTTP handlers.
#[derive(Clone)]
pub struct HttpState {
    /// Authentication use case for login flows.
    pub login: Arc<dyn LoginService>,
    /// Use case for listing users.
    pub users: Arc<dyn UsersQuery>,
    /// Use case for reading the current user's profile.
    pub profile: Arc<dyn UserProfileQuery>,
    /// Use case for updating a user's interest theme selections.
    pub interests: Arc<dyn UserInterestsCommand>,
    /// Use case for updating user preferences, guarded by revision checks.
    pub preferences: Arc<dyn UserPreferencesCommand>,
    /// Use case for fetching user preferences.
    pub preferences_query: Arc<dyn UserPreferencesQuery>,
    /// Use case for updating route annotations, guarded by revision checks.
    pub route_annotations: Arc<dyn RouteAnnotationsCommand>,
    /// Use case for fetching route annotations.
    pub route_annotations_query: Arc<dyn RouteAnnotationsQuery>,
    /// Use case for submitting route requests with idempotency support.
    pub route_submission: Arc<dyn RouteSubmissionService>,
    /// Port for reading Explore catalogue snapshots.
    pub catalogue: Arc<dyn CatalogueRepository>,
    /// Port for reading descriptor registries.
    pub descriptors: Arc<dyn DescriptorRepository>,
    /// Driving port for offline bundle mutations.
    pub offline_bundles: Arc<dyn OfflineBundleCommand>,
    /// Driving port for offline bundle reads.
    pub offline_bundles_query: Arc<dyn OfflineBundleQuery>,
    /// Port for persisting and reporting enrichment provenance.
    pub enrichment_provenance: Arc<dyn EnrichmentProvenanceRepository>,
    /// Driving port for walk session writes.
    pub walk_sessions: Arc<dyn WalkSessionCommand>,
    /// Driving port for walk session reads.
    pub walk_sessions_query: Arc<dyn WalkSessionQuery>,
    /// Recorder for pagination error observations, chosen by the composition root.
    pub pagination_error_metrics: Arc<dyn PaginationErrorMetrics>,
}

impl From<HttpStatePorts> for HttpState {
    fn from(ports: HttpStatePorts) -> Self {
        Self::new_with_extra(ports, HttpStateExtraPorts::default())
    }
}

impl HttpState {
    /// Construct state from a core ports bundle.
    ///
    /// # Examples
    /// ```no_run
    /// use std::sync::Arc;
    ///
    /// use backend::domain::ports::{
    ///     FixtureCatalogueRepository, FixtureDescriptorRepository, FixtureLoginService,
    ///     FixtureRouteAnnotationsCommand, FixtureRouteAnnotationsQuery,
    ///     FixtureRouteSubmissionService, FixtureUserInterestsCommand,
    ///     FixtureUserPreferencesCommand, FixtureUserPreferencesQuery, FixtureUserProfileQuery,
    ///     FixtureUsersQuery,
    /// };
    /// use backend::inbound::http::state::{HttpState, HttpStatePorts};
    ///
    /// let ports = HttpStatePorts {
    ///     login: Arc::new(FixtureLoginService),
    ///     users: Arc::new(FixtureUsersQuery),
    ///     profile: Arc::new(FixtureUserProfileQuery),
    ///     interests: Arc::new(FixtureUserInterestsCommand),
    ///     preferences: Arc::new(FixtureUserPreferencesCommand),
    ///     preferences_query: Arc::new(FixtureUserPreferencesQuery),
    ///     route_annotations: Arc::new(FixtureRouteAnnotationsCommand),
    ///     route_annotations_query: Arc::new(FixtureRouteAnnotationsQuery),
    ///     route_submission: Arc::new(FixtureRouteSubmissionService),
    ///     catalogue: Arc::new(FixtureCatalogueRepository),
    ///     descriptors: Arc::new(FixtureDescriptorRepository),
    /// };
    /// let state = HttpState::new(ports);
    /// let _login = state.login.clone();
    /// ```
    pub fn new(ports: HttpStatePorts) -> Self {
        Self::new_with_extra(ports, HttpStateExtraPorts::default())
    }

    /// Construct state from core and extra ports.
    ///
    /// # Examples
    /// ```no_run
    /// use std::sync::Arc;
    ///
    /// use backend::domain::ports::{
    ///     FixtureCatalogueRepository, FixtureDescriptorRepository, FixtureLoginService,
    ///     FixtureEnrichmentProvenanceRepository,
    ///     FixtureOfflineBundleCommand, FixtureOfflineBundleQuery,
    ///     FixtureRouteAnnotationsCommand, FixtureRouteAnnotationsQuery,
    ///     FixtureRouteSubmissionService, FixtureUserInterestsCommand,
    ///     FixtureUserPreferencesCommand, FixtureUserPreferencesQuery, FixtureUserProfileQuery,
    ///     FixtureUsersQuery, FixtureWalkSessionCommand, FixtureWalkSessionQuery,
    /// };
    /// use backend::inbound::http::state::{HttpState, HttpStateExtraPorts, HttpStatePorts};
    ///
    /// let ports = HttpStatePorts {
    ///     login: Arc::new(FixtureLoginService),
    ///     users: Arc::new(FixtureUsersQuery),
    ///     profile: Arc::new(FixtureUserProfileQuery),
    ///     interests: Arc::new(FixtureUserInterestsCommand),
    ///     preferences: Arc::new(FixtureUserPreferencesCommand),
    ///     preferences_query: Arc::new(FixtureUserPreferencesQuery),
    ///     route_annotations: Arc::new(FixtureRouteAnnotationsCommand),
    ///     route_annotations_query: Arc::new(FixtureRouteAnnotationsQuery),
    ///     route_submission: Arc::new(FixtureRouteSubmissionService),
    ///     catalogue: Arc::new(FixtureCatalogueRepository),
    ///     descriptors: Arc::new(FixtureDescriptorRepository),
    /// };
    /// let extras = HttpStateExtraPorts {
    ///     offline_bundles: Arc::new(FixtureOfflineBundleCommand),
    ///     offline_bundles_query: Arc::new(FixtureOfflineBundleQuery),
    ///     enrichment_provenance: Arc::new(FixtureEnrichmentProvenanceRepository),
    ///     walk_sessions: Arc::new(FixtureWalkSessionCommand),
    ///     walk_sessions_query: Arc::new(FixtureWalkSessionQuery),
    /// };
    /// let state = HttpState::new_with_extra(ports, extras);
    /// let _walk_sessions = state.walk_sessions.clone();
    /// ```
    pub fn new_with_extra(ports: HttpStatePorts, extras: HttpStateExtraPorts) -> Self {
        Self::new_with_extra_and_metrics(ports, extras, Arc::new(NoOpPaginationErrorMetrics))
    }

    /// Construct state with extra ports and the pagination-error metrics
    /// recorder selected by the application composition root.
    ///
    /// # Examples
    /// ```no_run
    /// use std::sync::Arc;
    ///
    /// use backend::domain::ports::{
    ///     NoOpPaginationErrorMetrics, PaginationErrorMetrics,
    /// };
    /// use backend::inbound::http::state::{
    ///     HttpState, HttpStateExtraPorts, HttpStatePorts,
    /// };
    ///
    /// # let ports: HttpStatePorts = todo!();
    /// # let extras: HttpStateExtraPorts = todo!();
    /// let metrics: Arc<dyn PaginationErrorMetrics> =
    ///     Arc::new(NoOpPaginationErrorMetrics);
    /// let state = HttpState::new_with_extra_and_metrics(ports, extras, metrics);
    /// let _ = state.pagination_error_metrics.clone();
    /// ```
    pub fn new_with_extra_and_metrics(
        ports: HttpStatePorts,
        extras: HttpStateExtraPorts,
        pagination_error_metrics: Arc<dyn PaginationErrorMetrics>,
    ) -> Self {
        let HttpStatePorts {
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
        } = ports;
        let HttpStateExtraPorts {
            offline_bundles,
            offline_bundles_query,
            enrichment_provenance,
            walk_sessions,
            walk_sessions_query,
        } = extras;
        Self {
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
            offline_bundles,
            offline_bundles_query,
            enrichment_provenance,
            walk_sessions,
            walk_sessions_query,
            pagination_error_metrics,
        }
    }
}
