//! Catalogue read-model domain types.
//!
//! These types model Explore and Discover snapshots as validated domain
//! entities owned by the domain layer.

use std::fmt;

use super::localization::LocalizationValidationError;
use super::semantic_icon_identifier::SemanticIconIdentifierValidationError;

mod community_pick;
mod image_asset;
mod route_category;
mod route_collection;
mod route_summary;
mod theme;
mod trending_route_highlight;
mod validation;

#[cfg(test)]
mod tests;

pub use community_pick::{CommunityPick, CommunityPickDraft};
pub use image_asset::ImageAsset;
pub use route_category::{RouteCategory, RouteCategoryDraft};
pub use route_collection::{RouteCollection, RouteCollectionDraft};
pub use route_summary::{RouteSummary, RouteSummaryDraft};
pub use theme::{Theme, ThemeDraft};
pub use trending_route_highlight::{TrendingRouteHighlight, TrendingRouteHighlightDraft};

/// Validation errors returned by catalogue read-model constructors.
#[derive(Debug, Clone, PartialEq)]
pub enum CatalogueValidationError {
    /// A slug field is empty, has surrounding whitespace, or fails the slug format.
    InvalidSlug {
        /// Dotted name of the field that failed, such as `route_summary.slug`.
        field: &'static str,
    },
    /// A text field is empty or contains only whitespace.
    EmptyField {
        /// Dotted name of the field that failed, such as `route_summary.slug`.
        field: &'static str,
    },
    /// A value that must not be negative was negative.
    NegativeValue {
        /// Dotted name of the field that failed, such as `route_summary.slug`.
        field: &'static str,
        /// The rejected negative value.
        value: i32,
    },
    /// A `[min, max]` range has a negative bound or `min` above `max`.
    InvalidRange {
        /// Dotted name of the field that failed, such as `route_summary.slug`.
        field: &'static str,
        /// Lower bound as supplied.
        min: i32,
        /// Upper bound as supplied.
        max: i32,
    },
    /// A rating falls outside the inclusive 0.0 to 5.0 range.
    InvalidRating {
        /// Dotted name of the field that failed, such as `route_summary.slug`.
        field: &'static str,
        /// The rejected rating as supplied.
        rating: f32,
    },
    /// A localization map failed validation.
    Localization(LocalizationValidationError),
    /// A semantic icon identifier failed validation.
    IconIdentifier(SemanticIconIdentifierValidationError),
}

impl fmt::Display for CatalogueValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSlug { field } => write!(
                f,
                "{field} must contain lowercase ASCII letters, digits, and hyphens"
            ),
            Self::EmptyField { field } => write!(f, "{field} must not be empty"),
            Self::NegativeValue { field, value } => {
                write!(f, "{field} must not be negative (got {value})")
            }
            Self::InvalidRange { field, min, max } => {
                write!(
                    f,
                    "{field} must have min <= max and both non-negative (got [{min}, {max}])"
                )
            }
            Self::InvalidRating { field, rating } => {
                write!(f, "{field} must be between 0.0 and 5.0 (got {rating})")
            }
            Self::Localization(error) => error.fmt(f),
            Self::IconIdentifier(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for CatalogueValidationError {}

impl From<LocalizationValidationError> for CatalogueValidationError {
    fn from(value: LocalizationValidationError) -> Self {
        Self::Localization(value)
    }
}

impl From<SemanticIconIdentifierValidationError> for CatalogueValidationError {
    fn from(value: SemanticIconIdentifierValidationError) -> Self {
        Self::IconIdentifier(value)
    }
}
