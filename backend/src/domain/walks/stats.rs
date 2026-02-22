//! Walk statistic types and constructors.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::WalkValidationError;

/// Primary walk-stat categories surfaced by the PWA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalkPrimaryStatKind {
    Distance,
    Duration,
}

/// Error returned when parsing a primary walk-stat kind from string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseWalkPrimaryStatKindError;

impl fmt::Display for WalkPrimaryStatKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Distance => f.write_str("distance"),
            Self::Duration => f.write_str("duration"),
        }
    }
}

impl fmt::Display for ParseWalkPrimaryStatKindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid walk primary stat kind")
    }
}

impl std::error::Error for ParseWalkPrimaryStatKindError {}

impl FromStr for WalkPrimaryStatKind {
    type Err = ParseWalkPrimaryStatKindError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "distance" => Ok(Self::Distance),
            "duration" => Ok(Self::Duration),
            _ => Err(ParseWalkPrimaryStatKindError),
        }
    }
}

/// Secondary walk-stat categories surfaced by the PWA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalkSecondaryStatKind {
    Energy,
    Count,
}

/// Error returned when parsing a secondary walk-stat kind from string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseWalkSecondaryStatKindError;

impl fmt::Display for WalkSecondaryStatKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Energy => f.write_str("energy"),
            Self::Count => f.write_str("count"),
        }
    }
}

impl fmt::Display for ParseWalkSecondaryStatKindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid walk secondary stat kind")
    }
}

impl std::error::Error for ParseWalkSecondaryStatKindError {}

impl FromStr for WalkSecondaryStatKind {
    type Err = ParseWalkSecondaryStatKindError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "energy" => Ok(Self::Energy),
            "count" => Ok(Self::Count),
            _ => Err(ParseWalkSecondaryStatKindError),
        }
    }
}

/// A primary completion statistic for a walk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "WalkPrimaryStatDraft")]
pub struct WalkPrimaryStat {
    kind: WalkPrimaryStatKind,
    value: f64,
}

/// Draft payload for a primary walk statistic.
///
/// # Examples
///
/// ```rust,ignore
/// let draft = backend::domain::WalkPrimaryStatDraft {
///     kind: backend::domain::WalkPrimaryStatKind::Distance,
///     value: 3600.0,
/// };
/// let stat = backend::domain::WalkPrimaryStat::try_from(draft)?;
/// assert_eq!(stat.value(), 3600.0);
/// Ok::<(), backend::domain::WalkValidationError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalkPrimaryStatDraft {
    pub kind: WalkPrimaryStatKind,
    pub value: f64,
}

impl WalkPrimaryStat {
    /// Creates a validated primary walk statistic.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let stat = backend::domain::WalkPrimaryStat::new(
    ///     backend::domain::WalkPrimaryStatKind::Distance,
    ///     3600.0,
    /// )?;
    /// assert_eq!(stat.kind(), backend::domain::WalkPrimaryStatKind::Distance);
    /// Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn new(kind: WalkPrimaryStatKind, value: f64) -> Result<Self, WalkValidationError> {
        if !value.is_finite() || value < 0.0 {
            return Err(WalkValidationError::NegativePrimaryStatValue { kind, value });
        }
        Ok(Self { kind, value })
    }

    /// Returns the statistic category.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let stat = sample_primary_stat()?;
    /// let _ = stat.kind();
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn kind(&self) -> WalkPrimaryStatKind {
        self.kind
    }

    /// Returns the statistic value.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let stat = sample_primary_stat()?;
    /// assert!(stat.value() >= 0.0);
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn value(&self) -> f64 {
        self.value
    }
}

/// A secondary completion statistic for a walk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "WalkSecondaryStatDraft")]
pub struct WalkSecondaryStat {
    kind: WalkSecondaryStatKind,
    value: f64,
    unit: Option<String>,
}

/// Draft payload for a secondary walk statistic.
///
/// # Examples
///
/// ```rust,ignore
/// let draft = backend::domain::WalkSecondaryStatDraft {
///     kind: backend::domain::WalkSecondaryStatKind::Energy,
///     value: 220.0,
///     unit: Some("kcal".to_owned()),
/// };
/// let stat = backend::domain::WalkSecondaryStat::try_from(draft)?;
/// assert_eq!(stat.unit(), Some("kcal"));
/// Ok::<(), backend::domain::WalkValidationError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalkSecondaryStatDraft {
    pub kind: WalkSecondaryStatKind,
    pub value: f64,
    pub unit: Option<String>,
}

impl WalkSecondaryStat {
    /// Creates a validated secondary walk statistic.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let stat = backend::domain::WalkSecondaryStat::new(
    ///     backend::domain::WalkSecondaryStatKind::Energy,
    ///     220.0,
    ///     Some("kcal".to_owned()),
    /// )?;
    /// assert_eq!(stat.unit(), Some("kcal"));
    /// Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn new(
        kind: WalkSecondaryStatKind,
        value: f64,
        unit: Option<String>,
    ) -> Result<Self, WalkValidationError> {
        if !value.is_finite() || value < 0.0 {
            return Err(WalkValidationError::NegativeSecondaryStatValue { kind, value });
        }

        if unit.as_deref().map(str::trim).is_some_and(str::is_empty) {
            return Err(WalkValidationError::EmptySecondaryStatUnit);
        }

        Ok(Self {
            kind,
            value,
            unit: unit.map(|v| v.trim().to_owned()),
        })
    }

    /// Returns the statistic category.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let stat = sample_secondary_stat()?;
    /// let _ = stat.kind();
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn kind(&self) -> WalkSecondaryStatKind {
        self.kind
    }

    /// Returns the statistic value.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let stat = sample_secondary_stat()?;
    /// assert!(stat.value() >= 0.0);
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Returns the optional normalized unit string.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let stat = sample_secondary_stat()?;
    /// let _ = stat.unit();
    /// # Ok::<(), backend::domain::WalkValidationError>(())
    /// ```
    pub fn unit(&self) -> Option<&str> {
        self.unit.as_deref()
    }
}
