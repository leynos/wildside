//! Overpass outbound adapters.
//!
//! This module provides a thin HTTP implementation of the
//! `OverpassEnrichmentSource` port.

mod dto;
mod http_source;

pub use http_source::{OverpassHttpIdentity, OverpassHttpSource};
