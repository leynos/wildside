//! Shared cache-control policies for HTTP handlers.

/// Private responses must always be revalidated before reuse.
pub const PRIVATE_NO_CACHE_MUST_REVALIDATE: &str = "private, no-cache, must-revalidate";

/// Build the standard cache-control header tuple for private API responses.
///
/// # Examples
///
/// ```no_run
/// use backend::inbound::http::cache_control::{
///     PRIVATE_NO_CACHE_MUST_REVALIDATE, private_no_cache_header,
/// };
///
/// assert_eq!(
///     private_no_cache_header(),
///     ("Cache-Control", PRIVATE_NO_CACHE_MUST_REVALIDATE)
/// );
/// ```
pub const fn private_no_cache_header() -> (&'static str, &'static str) {
    ("Cache-Control", PRIVATE_NO_CACHE_MUST_REVALIDATE)
}
