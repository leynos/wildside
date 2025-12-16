//! Shared HTTP adapter state.
//!
//! HTTP handlers accept this state via `actix_web::web::Data` so they only
//! depend on domain ports (use-cases) and remain testable without I/O.

use std::sync::Arc;

use crate::domain::ports::{LoginService, UsersQuery};

/// Dependency bundle for HTTP handlers.
#[derive(Clone)]
pub struct HttpState {
    pub login: Arc<dyn LoginService>,
    pub users: Arc<dyn UsersQuery>,
}

impl HttpState {
    /// Construct state from explicit port implementations.
    pub fn new(login: Arc<dyn LoginService>, users: Arc<dyn UsersQuery>) -> Self {
        Self { login, users }
    }
}
