//! Listing controls that only `test-support` builds can set.
//!
//! A production listing always reads one `REPEATABLE READ` snapshot. The
//! concurrent-listing control needs to change the row set between two pages of
//! the real `list_dead` loop, so this module adds a page hook and an isolation
//! override behind the crate's `test-support` feature. Both installers are
//! compiled out of every other build, and the adapter they configure is the
//! one under test rather than a copy of its query.

#[cfg(any(test, feature = "test-support"))]
use futures_util::future::BoxFuture;

#[cfg(any(test, feature = "test-support"))]
use super::PostgresDeadLetterQueue;

/// Hook run after every full listing page.
///
/// The argument is the number of pages read so far.
#[cfg(any(test, feature = "test-support"))]
pub type ListingPageProbe = std::sync::Arc<dyn Fn(usize) -> BoxFuture<'static, ()> + Send + Sync>;

/// Listing controls that `test-support` builds may set.
#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Default)]
pub(super) struct ListingControls {
    /// Isolation level replacing the production one.
    pub(super) isolation: Option<String>,
    /// Hook run after every full page.
    pub(super) page_probe: Option<ListingPageProbe>,
}

#[cfg(any(test, feature = "test-support"))]
impl PostgresDeadLetterQueue {
    /// Install a hook that runs after every full page of the listing.
    ///
    /// The concurrent-listing control uses it to change the row set between
    /// two pages of the real adapter loop.
    #[must_use]
    pub fn with_listing_page_probe<F>(mut self, probe: F) -> Self
    where
        F: Fn(usize) -> BoxFuture<'static, ()> + Send + Sync + 'static,
    {
        self.listing.page_probe = Some(std::sync::Arc::new(probe));
        self
    }

    /// Run the listing under a different isolation level.
    ///
    /// The negative control uses this: the same loop under `READ COMMITTED`
    /// takes a fresh snapshot for every statement.
    #[must_use]
    pub fn with_listing_isolation(mut self, isolation: impl Into<String>) -> Self {
        self.listing.isolation = Some(isolation.into());
        self
    }
}
