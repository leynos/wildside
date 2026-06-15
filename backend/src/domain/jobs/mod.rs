//! Domain job payloads dispatched through `RouteQueue`.
//!
//! Each job type uses a `#[serde(tag = "v")]` envelope so new versions can be
//! added without breaking older consumers. Worker handlers match on the
//! envelope and dispatch to the right schema.

pub mod bounding_box;
pub mod enrichment;
pub mod generate_route;

pub use bounding_box::{BoundingBox, BoundingBoxError};
pub use enrichment::{EnrichmentJob, EnrichmentJobBuildError, EnrichmentJobV1};
pub use generate_route::{GenerateRouteJob, GenerateRouteJobBuildError, GenerateRouteJobV1};
