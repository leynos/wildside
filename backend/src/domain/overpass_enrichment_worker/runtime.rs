//! Port and runtime dependency bundles for the enrichment worker.

use std::sync::Arc;

use crate::domain::ports::{EnrichmentJobMetrics, OsmPoiRepository, OverpassEnrichmentSource};

use super::{AttemptJitter, BackoffJitter, EnrichmentSleeper, TokioSleeper};

/// Port bundle required by the enrichment worker.
pub struct OverpassEnrichmentWorkerPorts {
    /// Outbound Overpass source adapter.
    pub source: Arc<dyn OverpassEnrichmentSource>,
    /// POI persistence adapter.
    pub poi_repository: Arc<dyn OsmPoiRepository>,
    /// Enrichment metrics adapter.
    pub metrics: Arc<dyn EnrichmentJobMetrics>,
}

impl OverpassEnrichmentWorkerPorts {
    /// Build a strongly-typed worker port bundle.
    pub fn new(
        source: Arc<dyn OverpassEnrichmentSource>,
        poi_repository: Arc<dyn OsmPoiRepository>,
        metrics: Arc<dyn EnrichmentJobMetrics>,
    ) -> Self {
        Self {
            source,
            poi_repository,
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
