//! Shared helpers for profile/interests startup-mode tests.
//!
//! These flows exercise the same login/session wiring across integration and
//! behavioural suites, so keeping the common constants and middleware builder
//! here avoids drift between test crates.

use actix_session::SessionMiddleware;
use actix_session::config::{CookieContentSecurity, PersistentSession};
use actix_session::storage::CookieSessionStore;
use actix_web::cookie::{Key, SameSite, time::Duration as CookieDuration};

pub const FIXTURE_AUTH_ID: &str = "123e4567-e89b-12d3-a456-426614174000";
pub const FIXTURE_PROFILE_NAME: &str = "Ada Lovelace";
pub const DB_PROFILE_NAME: &str = "Database Ada";
pub const FIRST_THEME_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";
pub const SECOND_THEME_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa7";
pub const INTEREST_THEME_IDS_MAX: usize = 100;

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
