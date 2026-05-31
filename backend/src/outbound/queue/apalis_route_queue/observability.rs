//! Queue observability instrumentation for enqueue operations.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use prometheus::{HistogramOpts, HistogramVec, IntCounterVec, Opts};
use tracing::warn;

const QUEUE_ENQUEUE_TOTAL: &str = "route_queue_enqueue_total";
const QUEUE_ENQUEUE_LATENCY_SECONDS: &str = "route_queue_enqueue_latency_seconds";

struct QueueMetrics {
    pub total: IntCounterVec,
    pub latency_seconds: HistogramVec,
}

static OBSERVABILITY: OnceLock<Result<QueueMetrics, String>> = OnceLock::new();
static METRICS_INIT_ERROR_REPORTED: AtomicBool = AtomicBool::new(false);

impl QueueMetrics {
    fn new() -> Result<Self, String> {
        let total = IntCounterVec::new(
            Opts::new(QUEUE_ENQUEUE_TOTAL, "Total route queue enqueue attempts"),
            &["outcome"],
        )
        .map_err(|error| format!("failed to create queue attempt counter: {error}"))?;

        let latency_seconds = HistogramVec::new(
            HistogramOpts::new(
                QUEUE_ENQUEUE_LATENCY_SECONDS,
                "Route queue enqueue latency by outcome in seconds",
            ),
            &["outcome"],
        )
        .map_err(|error| format!("failed to create queue enqueue latency histogram: {error}"))?;

        let registry = prometheus::default_registry();
        registry
            .register(Box::new(total.clone()))
            .map_err(|error| format!("failed to register queue attempt counter: {error}"))?;
        registry
            .register(Box::new(latency_seconds.clone()))
            .map_err(|error| format!("failed to register queue latency histogram: {error}"))?;

        Ok(Self {
            total,
            latency_seconds,
        })
    }
}

pub(super) fn observe_enqueue(outcome: &str, latency: Duration) {
    let metrics = OBSERVABILITY.get_or_init(QueueMetrics::new);
    match metrics {
        Ok(metrics) => {
            metrics.total.with_label_values(&[outcome]).inc();
            metrics
                .latency_seconds
                .with_label_values(&[outcome])
                .observe(latency.as_secs_f64());
        }
        Err(error) => {
            if METRICS_INIT_ERROR_REPORTED
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                warn!(error = %error, outcome, "queue observability metrics unavailable");
            }
        }
    }
}
