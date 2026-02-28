//! Port and runtime dependency bundles for the enrichment worker.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::domain::ports::{
    EnrichmentJobMetrics, EnrichmentProvenanceRepository, OsmPoiRepository,
    OverpassEnrichmentSource,
};

use super::{BackoffJitter, EnrichmentSleeper};

/// Port bundle required by the enrichment worker.
pub struct OverpassEnrichmentWorkerPorts {
    /// Outbound Overpass source adapter.
    pub source: Arc<dyn OverpassEnrichmentSource>,
    /// POI persistence adapter.
    pub poi_repository: Arc<dyn OsmPoiRepository>,
    /// Enrichment provenance persistence/listing adapter.
    pub provenance_repository: Arc<dyn EnrichmentProvenanceRepository>,
    /// Enrichment metrics adapter.
    pub metrics: Arc<dyn EnrichmentJobMetrics>,
}

impl OverpassEnrichmentWorkerPorts {
    /// Build a strongly-typed worker port bundle.
    pub fn new(
        source: Arc<dyn OverpassEnrichmentSource>,
        poi_repository: Arc<dyn OsmPoiRepository>,
        provenance_repository: Arc<dyn EnrichmentProvenanceRepository>,
        metrics: Arc<dyn EnrichmentJobMetrics>,
    ) -> Self {
        Self {
            source,
            poi_repository,
            provenance_repository,
            metrics,
        }
    }
}

/// Runtime helpers used by retry policy.
pub struct OverpassEnrichmentWorkerRuntime {
    /// Async sleep implementation.
    pub sleeper: Arc<dyn EnrichmentSleeper>,
    /// Jitter strategy for retry delays.
    pub jitter: Arc<dyn BackoffJitter>,
}

impl Default for OverpassEnrichmentWorkerRuntime {
    fn default() -> Self {
        Self {
            sleeper: Arc::new(TokioSleeper),
            jitter: Arc::new(AttemptJitter),
        }
    }
}

/// Tokio-based sleeper implementation.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioSleeper;

#[async_trait]
impl EnrichmentSleeper for TokioSleeper {
    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

/// Default deterministic jitter strategy.
#[derive(Debug, Clone, Copy, Default)]
pub struct AttemptJitter;

impl BackoffJitter for AttemptJitter {
    fn jittered_delay(&self, base: Duration, attempt: u32, now: DateTime<Utc>) -> Duration {
        let base_ms = u64::try_from(base.as_millis()).unwrap_or(u64::MAX);
        let max_extra = (base_ms / 4).max(1);
        let seed = u64::from(now.timestamp_subsec_nanos()) ^ u64::from(attempt);
        let extra = seed % (max_extra.saturating_add(1));
        Duration::from_millis(base_ms.saturating_add(extra))
    }
}
