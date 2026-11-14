//! Isolation control for the dead-list page walk.
//!
//! [`DeadLetterQueue::list_dead`] pages through `apalis.jobs` inside one
//! `REPEATABLE READ`, read-only snapshot. That choice only shows up if the row
//! set changes between two pages, which no ordinary scenario causes, so this
//! control makes the change itself. It installs the adapter's `test-support`
//! page hook, which runs after the first page, and deletes an already listed
//! row from a second pooled connection before the second page is read.
//!
//! The same listing is then run under `READ COMMITTED`, where every statement
//! takes a new snapshot. That negative control must lose the row that slid into
//! the second page; otherwise the assertion on the snapshot would hold
//! vacuously.
//!
//! [`DeadLetterQueue::list_dead`]: backend::domain::ports::DeadLetterQueue::list_dead

use std::future::Future;
use std::pin::Pin;

use apalis_postgres::PgPool;
use backend::domain::ports::{DeadJob, DeadLetterQueue};
use backend::outbound::queue::{PostgresDeadLetterQueue, RequeueBudgets};

use super::rows::JobSeed;
use super::{QUEUE_NAME, QueueDeadLetterWorld};

/// Number of dead rows seeded, spanning two listing pages.
const ROW_COUNT: i64 = 150;

/// Limit high enough that the walk continues until the rows run out.
const LISTING_LIMIT: u32 = 1000;

/// A deletion of one row that runs once, between the first and second page.
#[derive(Clone)]
struct DeferredDelete {
    pool: PgPool,
    id: String,
}

impl DeferredDelete {
    /// Return the page hook that deletes the row after the first full page.
    fn after_first_page(&self, pages_read: usize) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let pool = self.pool.clone();
        let id = self.id.clone();
        Box::pin(async move {
            if pages_read != 1 {
                return;
            }
            sqlx::query("DELETE FROM apalis.jobs WHERE id = $1")
                .bind(id)
                .execute(&pool)
                .await
                .expect("delete a listed row between pages");
        })
    }
}

impl QueueDeadLetterWorld {
    /// Seed `ROW_COUNT` dead rows and return their ids newest first.
    ///
    /// Ages increase by one second, so `done_at DESC` orders the rows exactly
    /// as they are seeded and the expected listing is a property of the seed
    /// rather than of insert timing.
    fn seed_listing_rows(&self) -> Vec<String> {
        (1..=ROW_COUNT)
            .map(|age| {
                self.insert_job(JobSeed {
                    job_type: QUEUE_NAME,
                    status: "Failed",
                    attempts: 3,
                    max_attempts: Some(3),
                    reason: Some("exhausted"),
                    last_error: Some("isolation row"),
                    done_age_seconds: age,
                })
                .as_str()
                .to_owned()
            })
            .collect()
    }

    /// List dead jobs through the adapter, deleting `victim` after page one.
    ///
    /// `isolation` replaces the adapter's own snapshot isolation when given,
    /// which is how the negative control runs the same loop under a level that
    /// takes a fresh snapshot per statement.
    fn list_around_deletion(&self, isolation: Option<&str>, victim: &str) -> Vec<DeadJob> {
        let deferred = DeferredDelete {
            pool: self.pool.clone(),
            id: victim.to_owned(),
        };
        let mut queue = PostgresDeadLetterQueue::new(
            self.pool.clone(),
            RequeueBudgets::from_policies([(QUEUE_NAME, self.policy.clone())]),
        )
        .with_listing_page_probe(move |pages_read| deferred.after_first_page(pages_read));
        if let Some(isolation) = isolation {
            queue = queue.with_listing_isolation(isolation);
        }
        self.runtime
            .block_on(queue.list_dead(LISTING_LIMIT))
            .expect("list dead jobs around a deletion")
    }
}

/// Scenario: a row is deleted after the first page of a listing has been read.
///
/// Invariant: the adapter's snapshot returns the whole listing with no skip,
/// while the same loop under read committed loses the row that slid into the
/// second page.
#[test]
fn page_walk_is_skip_free_only_under_one_snapshot() {
    let world = QueueDeadLetterWorld::new();
    let all = world.seed_listing_rows();
    let control_victim = all[10].clone();
    let snapshot_victim = all[20].clone();

    // Negative control first: read committed must show the defect the snapshot
    // prevents, otherwise the positive assertion proves nothing.
    let read_committed =
        listed_ids(&world.list_around_deletion(Some("READ COMMITTED"), &control_victim));
    assert!(
        read_committed.contains(&control_victim),
        "read committed read the victim on the first page, before it was deleted"
    );
    assert_eq!(
        read_committed.len(),
        all.len() - 1,
        "read committed returns every remaining row but skips the one that moved up"
    );
    assert!(
        !read_committed.contains(&all[100]),
        "read committed drops the row that slid into the second page"
    );

    // The adapter's own isolation must return that row instead.
    let expected: Vec<String> = all.into_iter().filter(|id| id != &control_victim).collect();
    let snapshot = listed_ids(&world.list_around_deletion(None, &snapshot_victim));
    assert_eq!(
        snapshot, expected,
        "one snapshot returns every row, including the one deleted mid-walk"
    );
    assert!(
        snapshot.contains(&snapshot_victim),
        "the snapshot still lists the row deleted after the first page"
    );
}

/// Collect the identifiers of a listing in the order the adapter returned them.
fn listed_ids(jobs: &[DeadJob]) -> Vec<String> {
    jobs.iter().map(|job| job.id.as_str().to_owned()).collect()
}
