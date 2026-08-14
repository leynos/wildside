//! Domain job payloads dispatched through `RouteQueue`.
//!
//! Each job type uses a `#[serde(tag = "v")]` envelope. The V1 wire shape is
//! stable, but V1-only consumers reject V2 with
//! [`JobDispatchError::Rejected`](crate::domain::ports::JobDispatchError::Rejected).
//! Producers must not send a version until consumers support it. Worker
//! handlers match on the envelope and dispatch to the right schema.

pub mod enrichment;
pub mod generate_route;

pub use crate::domain::bounding_box::{BoundingBox, BoundingBoxError};
pub use enrichment::{
    EnrichmentJob, EnrichmentJobBuildError, EnrichmentJobParams, EnrichmentJobV1,
};
pub use generate_route::{GenerateRouteJob, GenerateRouteJobV1};
