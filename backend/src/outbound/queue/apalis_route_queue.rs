//! Apalis-backed `RouteQueue` adapter using PostgreSQL storage.

use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use std::marker::PhantomData;
use std::time::Instant;

use apalis_core::backend::TaskSink;
use tracing::{instrument, warn};

use crate::domain::ports::{JobDispatchError, RouteQueue};

#[cfg(feature = "metrics")]
mod observability;

#[cfg(feature = "metrics")]
use observability::observe_enqueue;

#[cfg(not(feature = "metrics"))]
fn observe_enqueue(_outcome: &str, _latency: std::time::Duration) {}

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

    #[instrument(skip(self, plan))]
    async fn enqueue(&self, plan: &Self::Plan) -> Result<(), JobDispatchError> {
        // Serialize the plan to JSON value
        let payload = serde_json::to_value(plan).map_err(|error| {
            warn!(
                error = %error,
                "route queue serialisation failed"
            );
            JobDispatchError::rejected(format!("Failed to serialize plan: {error}"))
        })?;
        let started = Instant::now();

        // Push to the queue provider
        let result = self.provider.push_job(payload).await;
        let latency = started.elapsed();
        match &result {
            Ok(()) => {
                tracing::info!(
                    outcome = "success",
                    latency_ms = latency.as_millis(),
                    "route queue enqueue succeeded"
                );
            }
            Err(error) => {
                warn!(
                    error = %error,
                    outcome = "failure",
                    latency_ms = latency.as_millis(),
                    "route queue enqueue failed"
                );
            }
        }
        observe_enqueue(if result.is_ok() { "success" } else { "failure" }, latency);
        result
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
    #[instrument(skip(pool))]
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
            .map_err(|error| {
                warn!(error = %error, "route queue storage setup failed");
                JobDispatchError::unavailable(format!("Failed to setup Apalis tables: {error}"))
            })?;

        // Create the storage instance
        let storage = apalis_postgres::PostgresStorage::new(&pool);

        Ok(Self { storage })
    }
}

#[async_trait]
impl QueueProvider for ApalisPostgresProvider {
    #[instrument(skip(self, payload))]
    async fn push_job(&self, payload: Value) -> Result<(), JobDispatchError> {
        let mut storage = self.storage.clone();
        let started = Instant::now();
        storage.push(payload).await.map_err(|error| {
            warn!(
                error = %error,
                elapsed_ms = started.elapsed().as_millis(),
                "route queue push failed"
            );
            JobDispatchError::unavailable(format!("Failed to enqueue job: {error}"))
        })
    }
}

#[cfg(test)]
mod tests;
