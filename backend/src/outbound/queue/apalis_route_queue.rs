//! Apalis-backed `RouteQueue` adapter using PostgreSQL storage.

use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use std::marker::PhantomData;

use apalis_core::backend::TaskSink;

use crate::domain::ports::{JobDispatchError, RouteQueue};

/// Abstracts the queue storage backend for testability.
///
/// This trait allows the adapter to be tested with fake providers that don't
/// require a PostgreSQL connection, following the pattern established by
/// `RedisRouteCache`.
#[async_trait]
pub(crate) trait QueueProvider: Send + Sync {
    /// Pushes a serialized job payload into the queue.
    ///
    /// # Errors
    ///
    /// Returns `JobDispatchError::Unavailable` if the queue infrastructure is
    /// unreachable or returns an error. Returns `JobDispatchError::Rejected`
    /// if the job cannot be persisted (e.g., serialization failure).
    async fn push_job(&self, payload: Vec<u8>) -> Result<(), JobDispatchError>;
}

/// Apalis-backed `RouteQueue` adapter using PostgreSQL storage.
///
/// This adapter implements the `RouteQueue` port using `apalis-postgres` for
/// job persistence. It accepts typed plan payloads, serializes them to JSON,
/// and pushes them to the PostgreSQL-backed Apalis job table.
///
/// The adapter is generic over both the plan type `P` and the queue provider
/// `Q`, allowing unit tests to use fake providers while production uses the
/// real Apalis PostgreSQL storage.
///
/// # Type Parameters
///
/// - `P`: The plan type that will be enqueued. Must implement `Serialize` and
///   `DeserializeOwned` for persistence.
/// - `Q`: The queue provider that abstracts the Apalis storage backend.
///
/// # Example
///
/// ```rust,no_run
/// # use backend::outbound::queue::{GenericApalisRouteQueue, ApalisPostgresProvider};
/// # use backend::domain::ports::RouteQueue;
/// # use serde::{Serialize, Deserialize};
/// # use sqlx::PgPool;
/// #
/// # #[derive(Debug, Clone, Serialize, Deserialize)]
/// # struct MyPlan {
/// #     route_id: String,
/// # }
/// #
/// # async fn example(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
/// let provider = ApalisPostgresProvider::new(pool).await?;
/// let queue: GenericApalisRouteQueue<MyPlan, _> = GenericApalisRouteQueue::new(provider);
///
/// let plan = MyPlan { route_id: "route-123".to_string() };
/// queue.enqueue(&plan).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct GenericApalisRouteQueue<P, Q> {
    provider: Q,
    _plan: PhantomData<fn() -> P>,
}

impl<P, Q> GenericApalisRouteQueue<P, Q> {
    /// Creates a new Apalis route queue adapter with the given provider.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use backend::outbound::queue::{GenericApalisRouteQueue, ApalisPostgresProvider};
    /// # use serde::{Serialize, Deserialize};
    /// # use sqlx::PgPool;
    /// #
    /// # #[derive(Debug, Clone, Serialize, Deserialize)]
    /// # struct MyPlan;
    /// #
    /// # async fn example(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    /// let provider = ApalisPostgresProvider::new(pool).await?;
    /// let queue: GenericApalisRouteQueue<MyPlan, _> = GenericApalisRouteQueue::new(provider);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(provider: Q) -> Self {
        Self {
            provider,
            _plan: PhantomData,
        }
    }
}

/// Production type alias with the real Apalis PostgreSQL provider.
pub type ApalisRouteQueue<P> = GenericApalisRouteQueue<P, ApalisPostgresProvider>;

#[async_trait]
impl<P, Q> RouteQueue for GenericApalisRouteQueue<P, Q>
where
    P: Serialize + DeserializeOwned + Send + Sync,
    Q: QueueProvider,
{
    type Plan = P;

    async fn enqueue(&self, plan: &Self::Plan) -> Result<(), JobDispatchError> {
        // Serialize the plan to JSON bytes
        let payload = serde_json::to_vec(plan)
            .map_err(|e| JobDispatchError::rejected(format!("Failed to serialize plan: {e}")))?;

        // Push to the queue provider
        self.provider.push_job(payload).await
    }
}

/// Real provider backed by Apalis PostgreSQL storage.
///
/// This provider wraps `apalis_postgres::PostgresStorage` and maps its errors
/// to `JobDispatchError` variants.
#[derive(Clone)]
pub struct ApalisPostgresProvider {
    storage: apalis_postgres::PostgresStorage<serde_json::Value>,
}

impl ApalisPostgresProvider {
    /// Creates a new Apalis PostgreSQL provider.
    ///
    /// This method creates the Apalis job tables if they don't exist by calling
    /// `PostgresStorage::setup()`.
    ///
    /// # Errors
    ///
    /// Returns `JobDispatchError::Unavailable` if the database connection fails
    /// or if table creation fails.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use backend::outbound::queue::ApalisPostgresProvider;
    /// # use sqlx::PgPool;
    /// #
    /// # async fn example(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
    /// let provider = ApalisPostgresProvider::new(pool).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(pool: sqlx::PgPool) -> Result<Self, JobDispatchError> {
        // Create Apalis tables if they don't exist
        apalis_postgres::PostgresStorage::<(), (), ()>::setup(&pool)
            .await
            .map_err(|e| {
                JobDispatchError::unavailable(format!("Failed to setup Apalis tables: {e}"))
            })?;

        // Create the storage instance
        let storage = apalis_postgres::PostgresStorage::new(&pool);

        Ok(Self { storage })
    }
}

#[async_trait]
impl QueueProvider for ApalisPostgresProvider {
    async fn push_job(&self, payload: Vec<u8>) -> Result<(), JobDispatchError> {
        let job: serde_json::Value = serde_json::from_slice(&payload).map_err(|e| {
            JobDispatchError::rejected(format!("Failed to parse payload as JSON: {e}"))
        })?;

        let mut storage = self.storage.clone();
        storage
            .push(job)
            .await
            .map_err(|e| JobDispatchError::unavailable(format!("Failed to enqueue job: {e}")))
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the Apalis route queue adapter.
    use super::*;
    use crate::outbound::queue::test_helpers::{FailingQueueProvider, FakeQueueProvider};
    use rstest::rstest;
    use serde::{Deserialize, Serialize};

    /// Test plan type for unit tests.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct TestPlan {
        name: String,
    }

    #[rstest]
    #[tokio::test]
    async fn apalis_queue_enqueue_round_trips() {
        let fake_provider = FakeQueueProvider::new();
        let queue: GenericApalisRouteQueue<TestPlan, _> =
            GenericApalisRouteQueue::new(fake_provider.clone());

        let plan = TestPlan {
            name: "test-plan".to_string(),
        };

        let result = queue.enqueue(&plan).await;
        assert!(result.is_ok(), "enqueue should succeed with fake provider");

        let pushed_jobs = fake_provider.pushed_jobs();
        assert_eq!(pushed_jobs.len(), 1, "exactly one job should be pushed");

        // Verify the payload can be deserialized back to the original plan
        let deserialized: TestPlan =
            serde_json::from_slice(&pushed_jobs[0]).expect("pushed payload should be valid JSON");
        assert_eq!(
            deserialized, plan,
            "deserialized plan should match original"
        );
    }

    #[rstest]
    #[tokio::test]
    async fn apalis_queue_maps_provider_error_to_unavailable() {
        let failing_provider = FailingQueueProvider::new("simulated queue failure".to_string());
        let queue: GenericApalisRouteQueue<TestPlan, _> =
            GenericApalisRouteQueue::new(failing_provider);

        let plan = TestPlan {
            name: "test-plan".to_string(),
        };

        let result = queue.enqueue(&plan).await;
        assert!(
            result.is_err(),
            "enqueue should fail when provider returns error"
        );

        match result.unwrap_err() {
            JobDispatchError::Unavailable { message } => {
                assert!(
                    message.contains("simulated queue failure"),
                    "error message should contain provider error: {message}"
                );
            }
            JobDispatchError::Rejected { .. } => {
                panic!("expected Unavailable error, got Rejected");
            }
        }
    }

    #[rstest]
    #[tokio::test]
    async fn apalis_queue_enqueues_multiple_plans() {
        let fake_provider = FakeQueueProvider::new();
        let queue: GenericApalisRouteQueue<TestPlan, _> =
            GenericApalisRouteQueue::new(fake_provider.clone());

        let plan1 = TestPlan {
            name: "plan-1".to_string(),
        };
        let plan2 = TestPlan {
            name: "plan-2".to_string(),
        };

        queue
            .enqueue(&plan1)
            .await
            .expect("first enqueue should succeed");
        queue
            .enqueue(&plan2)
            .await
            .expect("second enqueue should succeed");

        let pushed_jobs = fake_provider.pushed_jobs();
        assert_eq!(pushed_jobs.len(), 2, "both jobs should be pushed");

        let deserialized1: TestPlan =
            serde_json::from_slice(&pushed_jobs[0]).expect("first payload should be valid JSON");
        let deserialized2: TestPlan =
            serde_json::from_slice(&pushed_jobs[1]).expect("second payload should be valid JSON");

        assert_eq!(deserialized1, plan1, "first plan should match");
        assert_eq!(deserialized2, plan2, "second plan should match");
    }
}
