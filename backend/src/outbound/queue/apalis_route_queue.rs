//! Apalis-backed `RouteQueue` adapter using PostgreSQL storage.

use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use std::marker::PhantomData;

use apalis_core::backend::TaskSink;

use crate::domain::ports::{JobDispatchError, RouteQueue};

/// Abstracts the queue storage backend for testability.
///
/// This trait allows the adapter to be tested with fake providers that don't
/// require a PostgreSQL connection, following the pattern established by
/// `RedisRouteCache`.
///
/// The API accepts a `serde_json::Value` so that higher-level components
/// (such as `GenericApalisRouteQueue`) perform a single serialization step,
/// while concrete providers remain decoupled from the specific payload type.
///
/// # Visibility
///
/// This trait is crate-internal to enable unit testing with fake providers
/// while keeping the abstraction private from external consumers.
#[async_trait]
pub(crate) trait QueueProvider: Send + Sync {
    /// Pushes a JSON job payload into the queue.
    ///
    /// Implementations are responsible for any storage-specific encoding
    /// (e.g. converting to bytes, SQL parameters, etc.).
    ///
    /// # Errors
    ///
    /// Returns `JobDispatchError::Unavailable` if the queue infrastructure is
    /// not reachable or otherwise cannot accept the job.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // QueueProvider is crate-internal; this example is for maintainers.
    /// use async_trait::async_trait;
    /// use serde_json::{json, Value};
    /// use backend::domain::ports::JobDispatchError;
    ///
    /// struct MyProvider;
    ///
    /// #[async_trait]
    /// impl QueueProvider for MyProvider {
    ///     async fn push_job(&self, payload: Value) -> Result<(), JobDispatchError> {
    ///         Ok(())
    ///     }
    /// }
    ///
    /// async fn example() -> Result<(), JobDispatchError> {
    ///     let provider = MyProvider;
    ///     let payload = json!({ "name": "route-123" });
    ///     provider.push_job(payload).await?;
    ///     Ok(())
    /// }
    /// ```
    async fn push_job(&self, payload: Value) -> Result<(), JobDispatchError>;
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
/// - `P`: The plan type that will be enqueued. Must implement `Serialize` for
///   persistence.
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
    P: Serialize + Send + Sync,
    Q: QueueProvider,
{
    type Plan = P;

    async fn enqueue(&self, plan: &Self::Plan) -> Result<(), JobDispatchError> {
        // Serialize the plan to JSON value
        let payload = serde_json::to_value(plan)
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
        // `setup()` is defined exclusively on `impl PostgresStorage<(), (), ()>`
        // by the apalis-postgres library (see `impl PostgresStorage<(), (), ()>`
        // in apalis-sql/src/postgres.rs). It is not available on any other type
        // parameter, so it cannot be called as
        // `PostgresStorage::<serde_json::Value>::setup(&pool)`.
        //
        // This is safe: Apalis uses a single shared `apalis.jobs` table for all
        // job types, not one table per type.  The `()` instantiation is a
        // library-mandated convention for running schema migrations only; the
        // `storage` field below operates on the correct `serde_json::Value` type
        // at runtime.
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
    async fn push_job(&self, payload: Value) -> Result<(), JobDispatchError> {
        let mut storage = self.storage.clone();
        storage
            .push(payload)
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

        let pushed_jobs = fake_provider
            .pushed_jobs()
            .expect("should be able to access pushed jobs");
        assert_eq!(pushed_jobs.len(), 1, "exactly one job should be pushed");

        // Verify the payload can be deserialized back to the original plan
        let deserialized: TestPlan = serde_json::from_value(pushed_jobs[0].clone())
            .expect("pushed payload should be valid JSON");
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

        match result.expect_err("expected error but call succeeded") {
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

        let pushed_jobs = fake_provider
            .pushed_jobs()
            .expect("should be able to access pushed jobs");
        assert_eq!(pushed_jobs.len(), 2, "both jobs should be pushed");

        let deserialized1: TestPlan = serde_json::from_value(pushed_jobs[0].clone())
            .expect("first payload should be valid JSON");
        let deserialized2: TestPlan = serde_json::from_value(pushed_jobs[1].clone())
            .expect("second payload should be valid JSON");

        assert_eq!(deserialized1, plan1, "first plan should match");
        assert_eq!(deserialized2, plan2, "second plan should match");
    }

    /// Test plan type that always fails serialization.
    #[derive(Debug, Clone)]
    struct FailingSerializePlan;

    impl Serialize for FailingSerializePlan {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom("simulated serialization failure"))
        }
    }

    #[rstest]
    #[tokio::test]
    async fn apalis_queue_maps_serialization_failure_to_rejected() {
        let fake_provider = FakeQueueProvider::new();
        let queue: GenericApalisRouteQueue<FailingSerializePlan, _> =
            GenericApalisRouteQueue::new(fake_provider.clone());

        let plan = FailingSerializePlan;

        let result = queue.enqueue(&plan).await;
        assert!(
            result.is_err(),
            "enqueue should fail when serialization fails"
        );

        match result.expect_err("expected error but call succeeded") {
            JobDispatchError::Rejected { message } => {
                assert!(
                    message.contains("Failed to serialize plan"),
                    "error message should contain adapter context: {message}"
                );
                assert!(
                    message.contains("simulated serialization failure"),
                    "error message should contain underlying serializer error: {message}"
                );
            }
            JobDispatchError::Unavailable { .. } => {
                panic!("expected Rejected error for serialization failure, got Unavailable");
            }
        }

        // Verify nothing was pushed to the provider
        let pushed_jobs = fake_provider
            .pushed_jobs()
            .expect("should be able to access pushed jobs");
        assert_eq!(
            pushed_jobs.len(),
            0,
            "no jobs should be pushed when serialization fails"
        );
    }
}
