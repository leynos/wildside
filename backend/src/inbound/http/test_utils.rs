//! Test helpers for inbound HTTP components.

use actix_session::{SessionMiddleware, storage::CookieSessionStore};
use actix_web::cookie::Key;

/// Build a session middleware configured for tests.
///
/// - Generates a fresh signing/encryption key per invocation.
/// - Sets the cookie name to `session` and disables the `Secure` flag for
///   local HTTP tests.
pub fn test_session_middleware() -> SessionMiddleware<CookieSessionStore> {
    SessionMiddleware::builder(CookieSessionStore::default(), Key::generate())
        .cookie_name("session".to_owned())
        .cookie_secure(false)
        .build()
}
