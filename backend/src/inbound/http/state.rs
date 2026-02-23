//! Shared HTTP adapter state.
//!
//! HTTP handlers accept this state via `actix_web::web::Data` so they only
//! depend on domain ports (use-cases) and remain testable without I/O.

use std::sync::Arc;

use crate::domain::ports::{
    CatalogueRepository, DescriptorRepository, LoginService, RouteAnnotationsCommand,
    RouteAnnotationsQuery, RouteSubmissionService, UserInterestsCommand, UserPreferencesCommand,
    UserPreferencesQuery, UserProfileQuery, UsersQuery,
};
use crate::domain::ports::{
    FixtureOfflineBundleCommand, FixtureOfflineBundleQuery, FixtureWalkSessionCommand,
    FixtureWalkSessionQuery, OfflineBundleCommand, OfflineBundleQuery, WalkSessionCommand,
    WalkSessionQuery,
};

/// Parameter object bundling all port implementations for HTTP handlers.
#[derive(Clone)]
pub struct HttpStatePorts {
    pub login: Arc<dyn LoginService>,
    pub users: Arc<dyn UsersQuery>,
    pub profile: Arc<dyn UserProfileQuery>,
    pub interests: Arc<dyn UserInterestsCommand>,
    pub preferences: Arc<dyn UserPreferencesCommand>,
    pub preferences_query: Arc<dyn UserPreferencesQuery>,
    pub route_annotations: Arc<dyn RouteAnnotationsCommand>,
    pub route_annotations_query: Arc<dyn RouteAnnotationsQuery>,
    pub route_submission: Arc<dyn RouteSubmissionService>,
    pub catalogue: Arc<dyn CatalogueRepository>,
    pub descriptors: Arc<dyn DescriptorRepository>,
}

/// Optional ports for endpoints introduced after the initial HTTP state shape.
#[derive(Clone)]
pub struct HttpStateExtraPorts {
    pub offline_bundles: Arc<dyn OfflineBundleCommand>,
    pub offline_bundles_query: Arc<dyn OfflineBundleQuery>,
    pub walk_sessions: Arc<dyn WalkSessionCommand>,
    pub walk_sessions_query: Arc<dyn WalkSessionQuery>,
}

impl Default for HttpStateExtraPorts {
    fn default() -> Self {
        Self {
            offline_bundles: Arc::new(FixtureOfflineBundleCommand),
            offline_bundles_query: Arc::new(FixtureOfflineBundleQuery),
            walk_sessions: Arc::new(FixtureWalkSessionCommand),
            walk_sessions_query: Arc::new(FixtureWalkSessionQuery),
        }
    }
}

/// Dependency bundle for HTTP handlers.
#[derive(Clone)]
pub struct HttpState {
    pub login: Arc<dyn LoginService>,
    pub users: Arc<dyn UsersQuery>,
    pub profile: Arc<dyn UserProfileQuery>,
    pub interests: Arc<dyn UserInterestsCommand>,
    pub preferences: Arc<dyn UserPreferencesCommand>,
    pub preferences_query: Arc<dyn UserPreferencesQuery>,
    pub route_annotations: Arc<dyn RouteAnnotationsCommand>,
    pub route_annotations_query: Arc<dyn RouteAnnotationsQuery>,
    pub route_submission: Arc<dyn RouteSubmissionService>,
    pub catalogue: Arc<dyn CatalogueRepository>,
    pub descriptors: Arc<dyn DescriptorRepository>,
    pub offline_bundles: Arc<dyn OfflineBundleCommand>,
    pub offline_bundles_query: Arc<dyn OfflineBundleQuery>,
    pub walk_sessions: Arc<dyn WalkSessionCommand>,
    pub walk_sessions_query: Arc<dyn WalkSessionQuery>,
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
    ///     walk_sessions: Arc::new(FixtureWalkSessionCommand),
    ///     walk_sessions_query: Arc::new(FixtureWalkSessionQuery),
    /// };
    /// let state = HttpState::new_with_extra(ports, extras);
    /// let _walk_sessions = state.walk_sessions.clone();
    /// ```
    pub fn new_with_extra(ports: HttpStatePorts, extras: HttpStateExtraPorts) -> Self {
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
            walk_sessions,
            walk_sessions_query,
        }
    }
}
