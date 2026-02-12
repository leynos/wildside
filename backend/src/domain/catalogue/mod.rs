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
    InvalidSlug {
        field: &'static str,
    },
    EmptyField {
        field: &'static str,
    },
    NegativeValue {
        field: &'static str,
        value: i32,
    },
    InvalidRange {
        field: &'static str,
        min: i32,
        max: i32,
    },
    InvalidRating {
        field: &'static str,
        rating: f32,
    },
    Localization(LocalizationValidationError),
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
