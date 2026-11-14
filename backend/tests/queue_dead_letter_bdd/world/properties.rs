//! Property coverage for dead-job listing across batch boundaries.
//!
//! The listing loop pages through raw SQL batches and drops rows whose storage
//! metadata the domain cannot represent. Scenarios pin a few named mixtures;
//! this test instead generates them, so the invariant is checked for many
//! combinations of malformed rows, valid rows, and limits, including mixtures
//! whose page boundaries fall inside a run of filtered rows. The same test
//! asserts that listing leaves every skipped row untouched, so reading the
//! dead-letter queue stays free of write side effects.

use backend::domain::ports::DeadJobId;
use proptest::prelude::*;

use super::rows::JobSeed;
use super::{QUEUE_NAME, QueueDeadLetterWorld};

/// Age spacing between adjacent interleaved rows.
///
/// Listing orders by `done_at` descending, so a wide gap keeps the expected
/// order a property of the seed rather than of how long the inserts took.
const AGE_SPACING_SECONDS: i64 = 10;

/// Offset that places a malformed row halfway between two interleaved slots.
///
/// Valid rows sit at exact multiples of [`AGE_SPACING_SECONDS`], so this offset
/// interleaves filtered rows with listed ones instead of collecting every
/// rejection at one end of the listing.
const MALFORMED_AGE_OFFSET_SECONDS: i64 = 5;

/// Age of the malformed row at `index`, interleaved between valid rows.
fn malformed_age(index: usize) -> i64 {
    let index = i64::try_from(index).expect("a generated row index fits in i64");
    AGE_SPACING_SECONDS * index + MALFORMED_AGE_OFFSET_SECONDS
}

impl QueueDeadLetterWorld {
    /// Insert `count` valid dead rows and return their ids with their ages.
    ///
    /// `stride` is the number of malformed slots left between consecutive
    /// valid rows. Widening it as the malformed population grows is what
    /// keeps the valid rows spread across the whole generated range, so batch
    /// boundaries fall between listed rows rather than only below them.
    fn insert_ordered_dead_rows(&self, count: usize, stride: usize) -> Vec<(DeadJobId, i64)> {
        let stride = i64::try_from(stride).expect("a generated stride fits in i64");
        (0..count)
            .map(|index| {
                let step = i64::try_from(index + 1).expect("a generated row index fits in i64");
                let age = AGE_SPACING_SECONDS * stride * step;
                let id = self.insert_job(JobSeed {
                    job_type: QUEUE_NAME,
                    status: "Failed",
                    attempts: 3,
                    max_attempts: Some(3),
                    reason: Some("exhausted"),
                    last_error: Some("property row"),
                    done_age_seconds: age,
                });
                (id, age)
            })
            .collect()
    }

    /// Insert one valid dead row whose storage age is `age` seconds.
    ///
    /// The deterministic boundary case needs ages chosen to place listed rows
    /// on both sides of known batch boundaries, so it seeds rows directly
    /// rather than deriving ages from a stride.
    fn insert_valid_dead_row_at(&self, age: i64) -> (DeadJobId, i64) {
        let id = self.insert_job(JobSeed {
            job_type: QUEUE_NAME,
            status: "Failed",
            attempts: 3,
            max_attempts: Some(3),
            reason: Some("exhausted"),
            last_error: Some("boundary row"),
            done_age_seconds: age,
        });
        (id, age)
    }

    /// Insert malformed dead rows whose ages interleave with the valid ones.
    ///
    /// Both rejection branches are represented: an unparsable requeue count
    /// and a negative attempt count. The returned identifiers let the caller
    /// check afterwards that listing did not modify the rows it skipped.
    fn insert_interleaved_malformed_rows(
        &self,
        malformed_requeue_counts: usize,
        malformed_attempts: usize,
    ) -> Vec<DeadJobId> {
        let requeue_rows = (0..malformed_requeue_counts)
            .map(|index| self.insert_malformed_requeue_count_dead_job_at(malformed_age(index)));
        let attempt_rows = (0..malformed_attempts)
            .map(|index| self.insert_negative_attempt_dead_job_at(malformed_age(index)));
        requeue_rows.chain(attempt_rows).collect()
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
    /// truncated to the limit, wherever the batch boundaries fall, and every
    /// column the snapshot reads on the skipped rows is unchanged.
    #[test]
    fn listing_is_the_valid_prefix_of_generated_rows(
        malformed_requeue_counts in 0usize..=140,
        malformed_attempts in 0usize..=140,
        valid in 0usize..=8,
        limit in 1u32..=12,
    ) {
        let world = QueueDeadLetterWorld::new();
        let malformed_ids = world
            .insert_interleaved_malformed_rows(malformed_requeue_counts, malformed_attempts);
        let before: Vec<_> = malformed_ids.iter().map(|id| world.snapshot(id)).collect();

        // Up to 280 malformed rows span more than two listing batches. Spacing
        // the valid rows by the average malformed run keeps some of them on
        // each side of every batch boundary instead of clustered at the newest
        // end, where a broken page walk would still find them all.
        let malformed_total = malformed_requeue_counts + malformed_attempts;
        let stride = 1 + malformed_total / valid.max(1);
        let mut inserted = world.insert_ordered_dead_rows(valid, stride);

        world.list_dead(limit);

        // Build the reference independently of insertion order: sort the valid
        // rows by stored age. A younger row has a smaller age, so ascending age
        // is the listing's newest-first order.
        inserted.sort_by_key(|(_, age)| *age);
        let wanted = usize::try_from(limit).expect("a generated limit fits in usize");
        let expected: Vec<String> = inserted
            .iter()
            .take(wanted)
            .map(|(id, _)| id.as_str().to_owned())
            .collect();
        prop_assert_eq!(world.listed_ids(), expected);

        let after: Vec<_> = malformed_ids.iter().map(|id| world.snapshot(id)).collect();
        prop_assert_eq!(after, before, "listing must not modify the rows it skips");
    }
}

/// Scenario: a fixed mixture whose listed rows straddle two batch boundaries.
///
/// The generated cases above may shrink to a mixture with every listed row in
/// one batch, so they cannot guarantee that the page walk is exercised with
/// listed rows on both sides of more than one boundary. This case fixes the
/// shape instead. Two populations of 125 malformed rows each span ages 5 to
/// 1245, which fills three 100-row batches; the four valid rows land at
/// positions 90, 131, 182 and 233, so the boundaries at 100 and 200 each fall
/// between a pair of listed rows.
///
/// Invariant: the listing is those four rows newest first, truncated by the
/// limit, and the rows it skips keep every column the snapshot reads.
#[test]
fn listing_crosses_two_batch_boundaries() {
    let world = QueueDeadLetterWorld::new();
    let malformed_ids = world.insert_interleaved_malformed_rows(125, 125);
    let before: Vec<_> = malformed_ids.iter().map(|id| world.snapshot(id)).collect();
    let inserted: Vec<_> = [452_i64, 652, 902, 1152]
        .into_iter()
        .map(|age| world.insert_valid_dead_row_at(age))
        .collect();
    let newest_first: Vec<String> = inserted
        .iter()
        .map(|(id, _)| id.as_str().to_owned())
        .collect();

    world.list_dead(4);
    assert_eq!(world.listed_ids(), newest_first);

    world.list_dead(2);
    assert_eq!(world.listed_ids(), newest_first[..2].to_vec());

    let after: Vec<_> = malformed_ids.iter().map(|id| world.snapshot(id)).collect();
    assert_eq!(after, before, "listing must not modify the rows it skips");
}
