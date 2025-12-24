//! Shared HTTP adapter state.
//!
//! HTTP handlers accept this state via `actix_web::web::Data` so they only
//! depend on domain ports (use-cases) and remain testable without I/O.

use std::sync::Arc;

use crate::domain::ports::{
    LoginService, RouteSubmissionService, UserInterestsCommand, UserProfileQuery, UsersQuery,
};

/// Parameter object bundling all port implementations for HTTP handlers.
#[derive(Clone)]
pub struct HttpStatePorts {
    pub login: Arc<dyn LoginService>,
    pub users: Arc<dyn UsersQuery>,
    pub profile: Arc<dyn UserProfileQuery>,
    pub interests: Arc<dyn UserInterestsCommand>,
    pub route_submission: Arc<dyn RouteSubmissionService>,
}

impl HttpStatePorts {
    /// Construct a new ports bundle.
    pub fn new(
        login: Arc<dyn LoginService>,
        users: Arc<dyn UsersQuery>,
        profile: Arc<dyn UserProfileQuery>,
        interests: Arc<dyn UserInterestsCommand>,
        route_submission: Arc<dyn RouteSubmissionService>,
    ) -> Self {
        Self {
            login,
            users,
            profile,
            interests,
            route_submission,
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
    pub route_submission: Arc<dyn RouteSubmissionService>,
}

impl HttpState {
    /// Construct state from a ports bundle.
    pub fn new(ports: HttpStatePorts) -> Self {
        let HttpStatePorts {
            login,
            users,
            profile,
            interests,
            route_submission,
        } = ports;
        Self {
            login,
            users,
            profile,
            interests,
            route_submission,
        }
    }
}
