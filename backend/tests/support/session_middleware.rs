//! Session middleware helpers shared by selected integration-test crates.

use actix_session::SessionMiddleware;
use actix_session::config::{CookieContentSecurity, PersistentSession};
use actix_session::storage::CookieSessionStore;
use actix_web::cookie::{Key, SameSite, time::Duration as CookieDuration};

/// Builds the cookie-backed [`SessionMiddleware`] used by integration tests.
///
/// The middleware stores sessions in a private (encrypted) `session` cookie
/// scoped to `/`, generated with a fresh random [`Key`] per call. It is marked
/// HTTP-only and `SameSite=Lax`, left insecure so it works over plain HTTP in
/// tests, and given a two-hour persistent lifetime.
///
/// # Returns
///
/// A [`SessionMiddleware`] backed by [`CookieSessionStore`], ready to wrap an
/// Actix Web application under test.
pub fn build_session_middleware() -> SessionMiddleware<CookieSessionStore> {
    SessionMiddleware::builder(CookieSessionStore::default(), Key::generate())
        .cookie_name("session".to_owned())
        .cookie_path("/".to_owned())
        .cookie_secure(false)
        .cookie_http_only(true)
        .cookie_content_security(CookieContentSecurity::Private)
        .cookie_same_site(SameSite::Lax)
        .session_lifecycle(PersistentSession::default().session_ttl(CookieDuration::hours(2)))
        .build()
}
