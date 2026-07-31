//! Versioned enrichment job payloads.

use std::fmt;

use chrono::{DateTime, Utc};
use serde::de::{Error as _, IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
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
#[derive(Clone, Debug, PartialEq, Serialize)]
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
    tags: EnrichmentTags,
    /// Wall-clock time at which the job was built and enqueued.
    enqueued_at: DateTime<Utc>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EnrichmentJobV1EnvelopeRaw {
    #[serde(rename = "v")]
    version: EnrichmentJobV1Version,
    job_id: Uuid,
    idempotency_key: Option<IdempotencyKey>,
    bounding_box: BoundingBox,
    tags: EnrichmentTags,
    enqueued_at: DateTime<Utc>,
}

struct EnrichmentJobV1Version;

impl<'de> Deserialize<'de> for EnrichmentJobV1Version {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(EnrichmentJobV1VersionVisitor)
    }
}

struct EnrichmentJobV1VersionVisitor;

impl Visitor<'_> for EnrichmentJobV1VersionVisitor {
    type Value = EnrichmentJobV1Version;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the supported enrichment job version v1")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if value == "v1" {
            Ok(EnrichmentJobV1Version)
        } else {
            Err(E::custom("unsupported enrichment job envelope version"))
        }
    }
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
        let tags = EnrichmentTags::try_from(params.tags)?;
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
                tags: payload.tags.0.clone(),
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
            Self::V1(payload) => &payload.tags.0,
        }
    }
}

impl<'de> Deserialize<'de> for EnrichmentJob {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = EnrichmentJobV1EnvelopeRaw::deserialize(deserializer)?;
        let EnrichmentJobV1EnvelopeRaw {
            version: EnrichmentJobV1Version,
            job_id,
            idempotency_key,
            bounding_box,
            tags,
            enqueued_at,
        } = raw;

        Ok(Self::V1(EnrichmentJobV1 {
            job_id,
            idempotency_key,
            bounding_box,
            tags,
            enqueued_at,
        }))
    }
}

#[derive(Clone, Debug, PartialEq)]
struct EnrichmentTags(Vec<String>);

impl TryFrom<Vec<String>> for EnrichmentTags {
    type Error = EnrichmentJobBuildError;

    fn try_from(tags: Vec<String>) -> Result<Self, Self::Error> {
        canonicalize_tags(tags).map(Self)
    }
}

impl Serialize for EnrichmentTags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for EnrichmentTags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(EnrichmentTagsVisitor)
    }
}

struct EnrichmentTagsVisitor;

impl<'de> Visitor<'de> for EnrichmentTagsVisitor {
    type Value = EnrichmentTags;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "between 1 and {ENRICHMENT_JOB_V1_MAX_TAGS} enrichment tags"
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let capacity = sequence
            .size_hint()
            .unwrap_or_default()
            .min(ENRICHMENT_JOB_V1_MAX_TAGS);
        let mut tags = Vec::with_capacity(capacity);

        while tags.len() < ENRICHMENT_JOB_V1_MAX_TAGS {
            let Some(tag) = sequence.next_element::<BoundedTag>()? else {
                return EnrichmentTags::try_from(tags).map_err(A::Error::custom);
            };
            tags.push(tag.0);
        }

        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom(EnrichmentJobBuildError::TooManyTags {
                limit: ENRICHMENT_JOB_V1_MAX_TAGS,
                observed: ENRICHMENT_JOB_V1_MAX_TAGS + 1,
            }));
        }

        EnrichmentTags::try_from(tags).map_err(A::Error::custom)
    }
}

struct BoundedTag(String);

impl<'de> Deserialize<'de> for BoundedTag {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(BoundedTagVisitor)
    }
}

struct BoundedTagVisitor;

impl<'de> Visitor<'de> for BoundedTagVisitor {
    type Value = BoundedTag;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "an enrichment tag no longer than {ENRICHMENT_JOB_V1_MAX_TAG_LENGTH} UTF-8 bytes"
        )
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        validate_tag_length(value).map_err(E::custom)?;
        Ok(BoundedTag(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        validate_tag_length(&value).map_err(E::custom)?;
        Ok(BoundedTag(value))
    }
}

fn validate_tag_length(tag: &str) -> Result<(), EnrichmentJobBuildError> {
    if tag.len() > ENRICHMENT_JOB_V1_MAX_TAG_LENGTH {
        return Err(EnrichmentJobBuildError::TagTooLong {
            limit: ENRICHMENT_JOB_V1_MAX_TAG_LENGTH,
            observed: tag.len(),
        });
    }
    Ok(())
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
        validate_tag_length(tag)?;
    }

    tags.sort();
    tags.dedup();
    Ok(tags)
}

#[cfg(test)]
mod tests;
