//! Property coverage for dead-job listing across batch boundaries.
//!
//! The listing loop pages through raw SQL batches and drops rows whose storage
//! metadata the domain cannot represent. Scenarios pin a few named mixtures;
//! this test instead generates them, so the invariant is checked for many
//! combinations of malformed rows, valid rows, and limits, including mixtures
//! whose page boundaries fall inside a run of filtered rows.

use backend::domain::ports::DeadJobId;
use proptest::prelude::*;

use super::rows::JobSeed;
use super::{QUEUE_NAME, QueueDeadLetterWorld};

/// Age spacing between valid rows.
///
/// Listing orders by `done_at` descending, so a wide gap keeps the expected
/// order a property of the seed rather than of how long the inserts took.
const AGE_SPACING_SECONDS: i64 = 10;

impl QueueDeadLetterWorld {
    /// Insert `count` valid dead rows, newest first, and return their ids.
    ///
    /// The malformed fixture is younger than every row inserted here, so an
    /// unfiltered listing would put those rows first and the comparison in the
    /// generated test would notice.
    fn insert_ordered_dead_rows(&self, count: usize) -> Vec<DeadJobId> {
        (0..count)
            .map(|index| {
                let age = AGE_SPACING_SECONDS
                    * i64::try_from(index + 1).expect("a generated row index fits in i64");
                self.insert_job(JobSeed {
                    job_type: QUEUE_NAME,
                    status: "Failed",
                    attempts: 3,
                    max_attempts: Some(3),
                    reason: Some("exhausted"),
                    last_error: Some("property row"),
                    done_age_seconds: age,
                })
            })
            .collect()
    }

    /// Return the identifiers from the most recent listing, in listed order.
    fn listed_ids(&self) -> Vec<String> {
        self.listing
            .lock()
            .expect("listing mutex")
            .iter()
            .map(|job| job.id.as_str().to_owned())
            .collect()
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 8, ..ProptestConfig::default() })]

    /// Scenario: a generated mixture of malformed and valid dead rows.
    ///
    /// Invariant: the listing is exactly the valid rows, newest first,
    /// truncated to the limit, wherever the batch boundaries fall.
    #[test]
    fn listing_is_the_valid_prefix_of_generated_rows(
        malformed in 0usize..130,
        valid in 0usize..8,
        limit in 1u32..=12,
    ) {
        let world = QueueDeadLetterWorld::new();
        for _ in 0..malformed {
            world.insert_malformed_requeue_count_dead_job();
        }
        let inserted = world.insert_ordered_dead_rows(valid);

        world.list_dead(limit);

        let wanted = usize::try_from(limit).expect("a generated limit fits in usize");
        let expected: Vec<String> = inserted
            .iter()
            .take(wanted)
            .map(|id| id.as_str().to_owned())
            .collect();
        prop_assert_eq!(world.listed_ids(), expected);
    }
}
