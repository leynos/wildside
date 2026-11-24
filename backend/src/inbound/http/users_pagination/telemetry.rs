//! Record users-page delivery from the links returned to the caller.
//!
//! This adapter boundary derives terminality from the emitted link that
//! continues the current traversal, then calls the port's logging `record`
//! method so metrics-free builds retain the same event.

use std::num::NonZeroUsize;

use actix_web::HttpRequest;
use pagination::PageParams;

use crate::domain::ports::{
    LimitSource, PageTraversal, PaginatedEndpoint, PaginationPageDelivery, PaginationPageMetrics,
    PaginationPageObservation, UsersPage,
};

/// Inputs required to build a users page response and record its delivery.
pub struct UsersPageResponse<'a> {
    /// The HTTP request used to construct absolute pagination links.
    pub request: &'a HttpRequest,
    /// Parsed pagination parameters used by the response envelope.
    pub params: &'a PageParams,
    /// The page returned by the users query port.
    pub page: UsersPage,
    /// How the request reached this page.
    pub traversal: PageTraversal,
    /// Whether the caller supplied the effective limit.
    pub limit_source: LimitSource,
    /// The validated, non-zero effective page limit.
    pub page_limit: NonZeroUsize,
    /// Recorder selected by the application composition root.
    pub metrics: &'a dyn PaginationPageMetrics,
}

/// Bounded facts derived from the users response delivered to the caller.
pub(super) struct UsersPageTelemetry<'a> {
    /// Recorder selected by the application composition root.
    pub(super) metrics: &'a dyn PaginationPageMetrics,
    /// How the caller reached the delivered page.
    pub(super) traversal: PageTraversal,
    /// Whether the caller supplied the effective limit.
    pub(super) limit_source: LimitSource,
    /// The validated, non-zero effective page limit.
    pub(super) page_limit: NonZeroUsize,
    /// Number of rows in the returned page before observation saturation.
    pub(super) returned_rows: usize,
    /// Whether the emitted response has a next link.
    pub(super) has_next_link: bool,
    /// Whether the emitted response has a previous link.
    pub(super) has_prev_link: bool,
}

/// Record one page after constructing the links returned to the caller.
pub(super) fn record_page_delivery(telemetry: &UsersPageTelemetry<'_>) {
    telemetry.metrics.record(page_observation(telemetry));
}

fn page_observation(telemetry: &UsersPageTelemetry<'_>) -> PaginationPageObservation {
    let continues_in_traversal_direction = match telemetry.traversal {
        PageTraversal::First | PageTraversal::Next => telemetry.has_next_link,
        PageTraversal::Prev => telemetry.has_prev_link,
    };

    PaginationPageObservation::new(
        PaginatedEndpoint::Users,
        telemetry.traversal,
        PaginationPageDelivery {
            limit_source: telemetry.limit_source,
            page_limit: telemetry.page_limit,
            returned_rows: telemetry.returned_rows,
            has_more: continues_in_traversal_direction,
        },
    )
}

#[cfg(test)]
mod tests {
    //! Verify telemetry follows the emitted link for the current traversal.

    use std::num::NonZeroUsize;

    use crate::domain::User;
    use crate::domain::ports::{
        LimitSource, MockPaginationPageMetrics, PageOutcome, PageTraversal,
        PaginationPageObservation,
    };
    use crate::inbound::http::users_pagination::boundary_cursor;
    use pagination::Direction;
    use rstest::rstest;

    use super::{UsersPageTelemetry, page_observation, record_page_delivery};

    fn sample_user() -> User {
        User::try_from_strings_at(
            "11111111-1111-1111-1111-111111111111",
            "Ada One",
            "2026-01-01T00:00:00Z"
                .parse()
                .expect("the sample timestamp is valid"),
        )
        .expect("the sample user is valid")
    }

    /// Compare the outcome with the actual cursor links for one dispatch case.
    #[rstest]
    #[case::first_next_continues(
        PageTraversal::First,
        Direction::Next,
        true,
        PageOutcome::Continuing
    )]
    #[case::first_next_terminal(
        PageTraversal::First,
        Direction::Next,
        false,
        PageOutcome::Terminal
    )]
    #[case::first_prev_uses_next_link(
        PageTraversal::First,
        Direction::Prev,
        true,
        PageOutcome::Continuing
    )]
    #[case::first_prev_terminal(
        PageTraversal::First,
        Direction::Prev,
        false,
        PageOutcome::Terminal
    )]
    #[case::next_next_continues(
        PageTraversal::Next,
        Direction::Next,
        true,
        PageOutcome::Continuing
    )]
    #[case::next_next_terminal(PageTraversal::Next, Direction::Next, false, PageOutcome::Terminal)]
    #[case::next_prev_uses_next_link(
        PageTraversal::Next,
        Direction::Prev,
        true,
        PageOutcome::Continuing
    )]
    #[case::next_prev_terminal(PageTraversal::Next, Direction::Prev, false, PageOutcome::Terminal)]
    #[case::prev_next_uses_prev_link(
        PageTraversal::Prev,
        Direction::Next,
        true,
        PageOutcome::Continuing
    )]
    #[case::prev_next_terminal(PageTraversal::Prev, Direction::Next, false, PageOutcome::Terminal)]
    #[case::prev_prev_continues(
        PageTraversal::Prev,
        Direction::Prev,
        true,
        PageOutcome::Continuing
    )]
    #[case::prev_prev_terminal(PageTraversal::Prev, Direction::Prev, false, PageOutcome::Terminal)]
    fn outcome_matches_emitted_traversal_link(
        #[case] traversal: PageTraversal,
        #[case] cursor_direction: Direction,
        #[case] has_more: bool,
        #[case] expected: PageOutcome,
    ) {
        let user = sample_user();
        let next_link = boundary_cursor(Some(&user), Direction::Next, traversal, has_more)
            .expect("the next boundary cursor should encode");
        let prev_link = boundary_cursor(Some(&user), Direction::Prev, traversal, has_more)
            .expect("the previous boundary cursor should encode");
        let selected_boundary_exists = match cursor_direction {
            Direction::Next => next_link.is_some(),
            Direction::Prev => prev_link.is_some(),
        };
        let expected_boundary_exists = match (traversal, cursor_direction) {
            (PageTraversal::First, Direction::Next) | (PageTraversal::Next, Direction::Next) => {
                has_more
            }
            (PageTraversal::First, Direction::Prev) => false,
            (PageTraversal::Next, Direction::Prev) | (PageTraversal::Prev, Direction::Next) => true,
            (PageTraversal::Prev, Direction::Prev) => has_more,
        };
        let relevant_link_exists = match traversal {
            PageTraversal::First | PageTraversal::Next => next_link.is_some(),
            PageTraversal::Prev => prev_link.is_some(),
        };
        let observation = page_observation(&UsersPageTelemetry {
            metrics: &crate::domain::ports::NoOpPaginationPageMetrics,
            traversal,
            limit_source: LimitSource::Explicit,
            page_limit: NonZeroUsize::new(2).expect("the test limit is non-zero"),
            returned_rows: 2,
            has_next_link: next_link.is_some(),
            has_prev_link: prev_link.is_some(),
        });

        assert_eq!(
            selected_boundary_exists, expected_boundary_exists,
            "the emitted boundary must match the dispatch table: {traversal:?}, {cursor_direction:?}, has_more={has_more}"
        );
        assert_eq!(
            relevant_link_exists,
            expected == PageOutcome::Continuing,
            "the case must expose the expected link for traversal {traversal:?} and cursor {cursor_direction:?} with has_more={has_more}"
        );
        assert_eq!(
            observation.outcome, expected,
            "outcome must follow the emitted link for traversal {traversal:?} and cursor {cursor_direction:?} with has_more={has_more}"
        );
        let is_first_page_with_backward_cursor_and_more =
            traversal == PageTraversal::First && cursor_direction == Direction::Prev && has_more;
        if is_first_page_with_backward_cursor_and_more {
            assert!(
                prev_link.is_none(),
                "the first page must omit its previous link even when another page exists"
            );
            assert_eq!(
                observation.outcome,
                PageOutcome::Continuing,
                "the first page outcome must follow its next link, not the absent previous link"
            );
        }
    }

    /// The delivery boundary must call the port's recording method once.
    #[rstest]
    #[case::first(PageTraversal::First, true, false)]
    #[case::next(PageTraversal::Next, true, false)]
    #[case::prev(PageTraversal::Prev, false, true)]
    fn record_page_delivery_calls_record_once(
        #[case] traversal: PageTraversal,
        #[case] has_next_link: bool,
        #[case] has_prev_link: bool,
    ) {
        let expected = PaginationPageObservation {
            endpoint: crate::domain::ports::PaginatedEndpoint::Users,
            traversal,
            outcome: PageOutcome::Continuing,
            limit_source: LimitSource::Explicit,
            page_limit: NonZeroUsize::new(2).expect("the test limit is non-zero"),
            returned_rows: 2,
        };
        let mut metrics = MockPaginationPageMetrics::new();
        metrics
            .expect_record()
            .withf(move |actual| *actual == expected)
            .times(1)
            .return_const(());

        let telemetry = UsersPageTelemetry {
            metrics: &metrics,
            traversal,
            limit_source: LimitSource::Explicit,
            page_limit: expected.page_limit,
            returned_rows: 2,
            has_next_link,
            has_prev_link,
        };
        record_page_delivery(&telemetry);
    }
}
