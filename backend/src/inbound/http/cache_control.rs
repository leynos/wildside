//! Shared cache-control policies for HTTP handlers.

/// Private responses must always be revalidated before reuse.
pub const PRIVATE_NO_CACHE_MUST_REVALIDATE: &str = "private, no-cache, must-revalidate";
