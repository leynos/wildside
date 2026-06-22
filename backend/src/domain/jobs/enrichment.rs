//! Versioned enrichment job payloads.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use crate::domain::IdempotencyKey;
use crate::domain::jobs::{BoundingBox, BoundingBoxError};
use crate::domain::ports::OverpassEnrichmentRequest;

/// Maximum number of tags carried on a V1 enrichment job.
pub const ENRICHMENT_JOB_V1_MAX_TAGS: usize = 64;
/// Maximum UTF-8 length in bytes of any single tag in V1.
pub const ENRICHMENT_JOB_V1_MAX_TAG_LENGTH: usize = 64;

/// Parameters for building a [`EnrichmentJob::V1`] payload.
pub struct EnrichmentJobParams {
    /// Stable job identifier for trace correlation.
    pub job_id: Uuid,
    /// Optional idempotency key supplied by the client.
    pub idempotency_key: Option<IdempotencyKey>,
    /// Validated WGS84 bounding box for enrichment.
    pub bounding_box: BoundingBox,
    /// Raw tag list to canonicalize into sorted, deduplicated form.
    pub tags: Vec<String>,
    /// Wall-clock time at which the job was built and enqueued.
    pub enqueued_at: DateTime<Utc>,
}

/// Versioned envelope for enrichment jobs.
///
/// Adding a field to an existing variant requires cutting a new `V2` variant.
/// Do not relax `deny_unknown_fields` on an existing variant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "v")]
pub enum EnrichmentJob {
    /// Version 1 enrichment payload.
    #[serde(rename = "v1")]
    V1(EnrichmentJobV1),
}

/// Version 1 payload for `EnrichmentJob`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnrichmentJobV1 {
    /// Stable job identifier for trace correlation.
    job_id: Uuid,
    /// Optional idempotency key supplied by the client.
    idempotency_key: Option<IdempotencyKey>,
    /// Validated WGS84 bounding box for enrichment.
    bounding_box: BoundingBox,
    /// Sorted, deduplicated tag list.
    tags: Vec<String>,
    /// Wall-clock time at which the job was built and enqueued.
    enqueued_at: DateTime<Utc>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EnrichmentJobV1Raw {
    job_id: Uuid,
    idempotency_key: Option<IdempotencyKey>,
    bounding_box: BoundingBox,
    tags: Vec<String>,
    enqueued_at: DateTime<Utc>,
}

/// Errors raised while building enrichment jobs.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EnrichmentJobBuildError {
    /// Bounding-box validation failed.
    #[error(transparent)]
    BoundingBox(#[from] BoundingBoxError),
    /// At least one tag is required.
    #[error("enrichment job requires at least one tag")]
    EmptyTags,
    /// The tag vector exceeds the V1 limit.
    #[error("enrichment job has too many tags: {observed} > {limit}")]
    TooManyTags { limit: usize, observed: usize },
    /// One tag exceeds the V1 byte-length limit.
    #[error("enrichment job tag is too long: {observed} > {limit}")]
    TagTooLong { limit: usize, observed: usize },
}

impl EnrichmentJob {
    /// Build a V1 enrichment job from validated pieces.
    pub fn v1(params: EnrichmentJobParams) -> Result<Self, EnrichmentJobBuildError> {
        let tags = canonicalize_tags(params.tags)?;
        Ok(Self::V1(EnrichmentJobV1 {
            job_id: params.job_id,
            idempotency_key: params.idempotency_key,
            bounding_box: params.bounding_box,
            tags,
            enqueued_at: params.enqueued_at,
        }))
    }

    /// Convert any envelope variant into the existing Overpass port request.
    pub fn to_overpass_request(&self) -> OverpassEnrichmentRequest {
        match self {
            Self::V1(payload) => OverpassEnrichmentRequest {
                job_id: payload.job_id,
                bounding_box: payload.bounding_box.coords(),
                tags: payload.tags.clone(),
            },
        }
    }

    /// Return the validated bounding box carried by this job.
    pub fn bounding_box(&self) -> BoundingBox {
        match self {
            Self::V1(payload) => payload.bounding_box,
        }
    }

    /// Return the canonical tag list carried by this job.
    pub fn tags(&self) -> &[String] {
        match self {
            Self::V1(payload) => payload.tags.as_slice(),
        }
    }
}

impl<'de> Deserialize<'de> for EnrichmentJobV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = EnrichmentJobV1Raw::deserialize(deserializer)?;
        let tags = canonicalize_tags(raw.tags).map_err(serde::de::Error::custom)?;

        Ok(Self {
            job_id: raw.job_id,
            idempotency_key: raw.idempotency_key,
            bounding_box: raw.bounding_box,
            tags,
            enqueued_at: raw.enqueued_at,
        })
    }
}

fn canonicalize_tags(mut tags: Vec<String>) -> Result<Vec<String>, EnrichmentJobBuildError> {
    if tags.is_empty() {
        return Err(EnrichmentJobBuildError::EmptyTags);
    }
    if tags.len() > ENRICHMENT_JOB_V1_MAX_TAGS {
        return Err(EnrichmentJobBuildError::TooManyTags {
            limit: ENRICHMENT_JOB_V1_MAX_TAGS,
            observed: tags.len(),
        });
    }

    for tag in &tags {
        if tag.len() > ENRICHMENT_JOB_V1_MAX_TAG_LENGTH {
            return Err(EnrichmentJobBuildError::TagTooLong {
                limit: ENRICHMENT_JOB_V1_MAX_TAG_LENGTH,
                observed: tag.len(),
            });
        }
    }

    tags.sort();
    tags.dedup();
    Ok(tags)
}

#[cfg(test)]
mod tests;
