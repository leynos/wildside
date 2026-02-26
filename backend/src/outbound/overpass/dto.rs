//! DTOs for decoding Overpass JSON responses.
//!
//! The adapter decodes into these transport DTOs first, then maps into domain
//! records (`OverpassPoi`) in one pass.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::domain::ports::OverpassPoi;

#[derive(Debug, Deserialize)]
pub(super) struct OverpassResponseDto {
    #[serde(default)]
    pub(super) elements: Vec<OverpassElementDto>,
}

#[derive(Debug, Deserialize)]
pub(super) struct OverpassElementDto {
    #[serde(rename = "type")]
    pub(super) element_type: String,
    pub(super) id: i64,
    pub(super) lon: Option<f64>,
    pub(super) lat: Option<f64>,
    pub(super) center: Option<OverpassElementCenterDto>,
    #[serde(default)]
    pub(super) tags: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct OverpassElementCenterDto {
    pub(super) lon: f64,
    pub(super) lat: f64,
}

impl OverpassResponseDto {
    pub(super) fn into_domain_pois(self) -> Result<Vec<OverpassPoi>, String> {
        self.elements
            .into_iter()
            .map(OverpassElementDto::into_domain_poi)
            .collect()
    }
}

impl OverpassElementDto {
    fn into_domain_poi(self) -> Result<OverpassPoi, String> {
        let (longitude, latitude) = self.coordinates().ok_or_else(|| {
            format!(
                "element {} ({}) missing coordinates",
                self.id, self.element_type
            )
        })?;
        if !longitude.is_finite() || !latitude.is_finite() {
            return Err(format!(
                "element {} ({}) includes non-finite coordinates",
                self.id, self.element_type
            ));
        }

        Ok(OverpassPoi {
            element_type: self.element_type,
            element_id: self.id,
            longitude,
            latitude,
            tags: self.tags,
        })
    }

    fn coordinates(&self) -> Option<(f64, f64)> {
        if let (Some(longitude), Some(latitude)) = (self.lon, self.lat) {
            return Some((longitude, latitude));
        }
        self.center.as_ref().map(|center| (center.lon, center.lat))
    }
}
