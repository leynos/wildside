//! Validated bounding-box payloads shared by background jobs.

/// WGS84 bounding-box validation errors.
#[derive(Debug, Clone, PartialEq)]
pub enum BoundingBoxError {}

/// WGS84 bounding box in `[min_lng, min_lat, max_lng, max_lat]` order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundingBox {
    coords: [f64; 4],
}

impl BoundingBox {
    /// Return the stored coordinates in wire-format order.
    pub fn coords(&self) -> [f64; 4] {
        self.coords
    }
}
