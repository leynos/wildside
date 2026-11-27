//! Port for recording successful pagination responses and structured events.
//!
//! The domain owns the closed label vocabularies and observation invariant so
//! HTTP and metrics adapters share the same bounded telemetry contract.

use std::num::NonZeroUsize;

use tracing::debug;

use crate::domain::TraceId;

/// Endpoint identity for pagination telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaginatedEndpoint {
    /// The users-list endpoint.
    Users,
}

impl PaginatedEndpoint {
    /// Return the stable label used for this endpoint.
    ///
    /// # Examples
    /// ```
    /// use backend::domain::ports::PaginatedEndpoint;
    ///
    /// assert_eq!(
    ///     PaginatedEndpoint::Users.as_label(),
    ///     "users",
    ///     "the users endpoint must keep its stable label"
    /// );
    /// ```
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Users => "users",
        }
    }
}

/// How the caller reached the current page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PageTraversal {
    /// The initial page request.
    First,
    /// A page reached by following the next link.
    Next,
    /// A page reached by following the previous link.
    Prev,
}

impl PageTraversal {
    /// Return the stable label used for this traversal.
    ///
    /// # Examples
    /// ```
    /// use backend::domain::ports::PageTraversal;
    ///
    /// assert_eq!(
    ///     PageTraversal::Prev.as_label(),
    ///     "prev",
    ///     "previous-page requests must use the prev label"
    /// );
    /// ```
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::First => "first",
            Self::Next => "next",
            Self::Prev => "prev",
        }
    }
}

/// Whether another page exists in the direction of travel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PageOutcome {
    /// A boundary link exists in the direction of travel.
    Continuing,
    /// No boundary link exists in the direction of travel.
    Terminal,
}

impl PageOutcome {
    /// Return the stable label used for this outcome.
    ///
    /// # Examples
    /// ```
    /// use backend::domain::ports::PageOutcome;
    ///
    /// assert_eq!(
    ///     PageOutcome::Terminal.as_label(),
    ///     "terminal",
    ///     "a missing boundary link must use the terminal label"
    /// );
    /// ```
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Continuing => "continuing",
            Self::Terminal => "terminal",
        }
    }
}

/// Provenance of the effective page size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LimitSource {
    /// The request omitted the limit and the server selected its default.
    Default,
    /// The request supplied a limit that was accepted as-is.
    Explicit,
    /// The request supplied a limit that the server clamped.
    Clamped,
}

impl LimitSource {
    /// Return the stable label used for this limit source.
    ///
    /// # Examples
    /// ```
    /// use backend::domain::ports::LimitSource;
    ///
    /// assert_eq!(
    ///     LimitSource::Default.as_label(),
    ///     "default",
    ///     "an omitted request limit must use the default label"
    /// );
    /// ```
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Explicit => "explicit",
            Self::Clamped => "clamped",
        }
    }
}

/// Page facts passed into the observation constructor.
///
/// This groups the effective limit, returned-row count, and continuation
/// signal reported by the HTTP page response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaginationPageDelivery {
    /// Provenance of the effective page size.
    pub limit_source: LimitSource,
    /// The effective page size, which is always non-zero.
    pub page_limit: NonZeroUsize,
    /// The row count reported by the page response before saturation.
    pub returned_rows: usize,
    /// Whether the response exposes another page in the direction of travel.
    pub has_more: bool,
}

/// The bounded facts recorded for one delivered page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaginationPageObservation {
    /// The endpoint that returned the page.
    pub endpoint: PaginatedEndpoint,
    /// How the caller reached the page.
    pub traversal: PageTraversal,
    /// Whether another page exists in the direction of travel.
    pub outcome: PageOutcome,
    /// Why the effective limit has its value.
    pub limit_source: LimitSource,
    /// The effective page size, which is always non-zero.
    pub page_limit: NonZeroUsize,
    /// The number of rows returned after saturation at `page_limit`.
    pub returned_rows: usize,
}

impl PaginationPageObservation {
    /// Construct an observation, saturating inconsistent row counts safely.
    ///
    /// Returned rows are capped at the effective page limit. A short page is
    /// terminal even if its caller reports another page, since that
    /// combination indicates a server-side pagination inconsistency.
    ///
    /// # Examples
    /// ```
    /// use std::num::NonZeroUsize;
    ///
    /// use backend::domain::ports::{
    ///     LimitSource, PageOutcome, PageTraversal, PaginationPageDelivery,
    ///     PaginationPageObservation, PaginatedEndpoint,
    /// };
    ///
    /// let observation = PaginationPageObservation::new(
    ///     PaginatedEndpoint::Users,
    ///     PageTraversal::Next,
    ///     PaginationPageDelivery {
    ///         limit_source: LimitSource::Explicit,
    ///         page_limit: NonZeroUsize::new(2).expect("the example limit is non-zero"),
    ///         returned_rows: 3,
    ///         has_more: true,
    ///     },
    /// );
    ///
    /// assert_eq!(
    ///     observation.returned_rows,
    ///     2,
    ///     "the observation must cap rows at the effective page limit"
    /// );
    /// assert_eq!(
    ///     observation.outcome,
    ///     PageOutcome::Continuing,
    ///     "a full page may retain its continuation"
    /// );
    /// ```
    pub fn new(
        endpoint: PaginatedEndpoint,
        traversal: PageTraversal,
        delivery: PaginationPageDelivery,
    ) -> Self {
        let returned_rows = delivery.returned_rows.min(delivery.page_limit.get());
        let has_more = delivery.has_more && returned_rows == delivery.page_limit.get();
        let outcome = if has_more {
            PageOutcome::Continuing
        } else {
            PageOutcome::Terminal
        };

        Self {
            endpoint,
            traversal,
            outcome,
            limit_source: delivery.limit_source,
            page_limit: delivery.page_limit,
            returned_rows,
        }
    }
}

/// Records page deliveries without coupling the domain to an exporter.
///
/// Call [`record`](Self::record) at a delivery boundary, never `increment`:
/// `record` emits the structured event before delegating. Direct `increment`
/// calls update metrics while silently dropping the event in every build.
#[cfg_attr(test, mockall::automock)]
pub trait PaginationPageMetrics: Send + Sync {
    /// Increment the page counters for an observation.
    ///
    /// Implementations must be synchronous and infallible so telemetry cannot
    /// change request handling. Call [`record`](Self::record) at call sites.
    ///
    /// # Examples
    /// ```
    /// use backend::domain::ports::{
    ///     LimitSource, NoOpPaginationPageMetrics, PageTraversal,
    ///     PaginationPageDelivery, PaginationPageMetrics, PaginationPageObservation,
    ///     PaginatedEndpoint,
    /// };
    ///
    /// let observation = PaginationPageObservation::new(
    ///     PaginatedEndpoint::Users,
    ///     PageTraversal::First,
    ///     PaginationPageDelivery {
    ///         limit_source: LimitSource::Default,
    ///         page_limit: std::num::NonZeroUsize::new(10)
    ///             .expect("the example limit is non-zero"),
    ///         returned_rows: 4,
    ///         has_more: false,
    ///     },
    /// );
    ///
    /// NoOpPaginationPageMetrics.increment(observation);
    /// ```
    fn increment(&self, observation: PaginationPageObservation);

    /// Emit the structured event, then record the observation.
    ///
    /// The event carries bounded labels, the effective limit, row count, and
    /// the current trace identifier when one is available.
    ///
    /// # Examples
    /// ```
    /// use backend::domain::ports::{
    ///     LimitSource, NoOpPaginationPageMetrics, PageTraversal,
    ///     PaginationPageDelivery, PaginationPageMetrics, PaginationPageObservation,
    ///     PaginatedEndpoint,
    /// };
    ///
    /// let observation = PaginationPageObservation::new(
    ///     PaginatedEndpoint::Users,
    ///     PageTraversal::First,
    ///     PaginationPageDelivery {
    ///         limit_source: LimitSource::Default,
    ///         page_limit: std::num::NonZeroUsize::new(10)
    ///             .expect("the example limit is non-zero"),
    ///         returned_rows: 4,
    ///         has_more: false,
    ///     },
    /// );
    ///
    /// NoOpPaginationPageMetrics.record(observation);
    /// ```
    fn record(&self, observation: PaginationPageObservation) {
        let trace_id = TraceId::current().map(|id| id.to_string());
        debug!(
            endpoint = observation.endpoint.as_label(),
            traversal = observation.traversal.as_label(),
            outcome = observation.outcome.as_label(),
            limit_source = observation.limit_source.as_label(),
            page_limit = observation.page_limit.get(),
            rows = observation.returned_rows,
            trace_id = trace_id.as_deref().unwrap_or("unknown"),
            "pagination page delivered"
        );
        self.increment(observation);
    }
}

/// No-op recorder that still emits the structured page event.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpPaginationPageMetrics;

impl PaginationPageMetrics for NoOpPaginationPageMetrics {
    fn increment(&self, _observation: PaginationPageObservation) {}
}
