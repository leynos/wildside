//! Validation helpers shared by catalogue read-model entities.

use super::CatalogueValidationError;
use crate::domain::slug::is_valid_slug;

pub(super) fn validate_slug(
    value: String,
    field: &'static str,
) -> Result<String, CatalogueValidationError> {
    if !is_valid_slug(&value) {
        return Err(CatalogueValidationError::InvalidSlug { field });
    }
    Ok(value)
}

pub(super) fn validate_non_empty_field(
    value: String,
    field: &'static str,
) -> Result<String, CatalogueValidationError> {
    if value.trim().is_empty() {
        return Err(CatalogueValidationError::EmptyField { field });
    }
    Ok(value)
}

pub(super) fn ensure_non_negative(
    value: i32,
    field: &'static str,
) -> Result<(), CatalogueValidationError> {
    if value < 0 {
        return Err(CatalogueValidationError::NegativeValue { field, value });
    }
    Ok(())
}

pub(super) fn ensure_non_negative_range(
    range: [i32; 2],
    field: &'static str,
) -> Result<(), CatalogueValidationError> {
    let [min, max] = range;
    let has_negative_bound = min < 0 || max < 0;
    if has_negative_bound || min > max {
        return Err(CatalogueValidationError::InvalidRange { field, min, max });
    }
    Ok(())
}

pub(super) fn ensure_valid_rating(
    rating: f32,
    field: &'static str,
) -> Result<(), CatalogueValidationError> {
    if !(0.0..=5.0).contains(&rating) {
        return Err(CatalogueValidationError::InvalidRating { field, rating });
    }
    Ok(())
}
