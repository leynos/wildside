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
}

impl From<HttpStatePorts> for HttpState {
    fn from(ports: HttpStatePorts) -> Self {
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
        }
    }
}

impl HttpState {
    /// Construct state from a ports bundle.
    pub fn new(ports: HttpStatePorts) -> Self {
        ports.into()
    }
}
