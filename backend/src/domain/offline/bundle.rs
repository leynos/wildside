//! Offline bundle entities and draft payloads.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::UserId;

use super::{
    BoundingBox, OfflineBundleKind, OfflineBundleStatus, OfflineValidationError, ZoomRange,
};

/// Input payload for [`OfflineBundle::new`].
///
/// # Examples
///
/// ```rust,ignore
/// let now = chrono::Utc::now();
/// let draft = backend::domain::OfflineBundleDraft {
///     id: uuid::Uuid::new_v4(),
///     owner_user_id: None,
///     device_id: "ios-phone".to_owned(),
///     kind: backend::domain::OfflineBundleKind::Route,
///     route_id: Some(uuid::Uuid::new_v4()),
///     region_id: None,
///     bounds: backend::domain::BoundingBox::new(-3.25, 55.92, -3.10, 56.01)?,
///     zoom_range: backend::domain::ZoomRange::new(12, 16)?,
///     estimated_size_bytes: 12_000_000,
///     created_at: now,
///     updated_at: now,
///     status: backend::domain::OfflineBundleStatus::Queued,
///     progress: 0.0,
/// };
/// let bundle = backend::domain::OfflineBundle::new(draft)?;
/// assert_eq!(bundle.progress(), 0.0);
/// Ok::<(), backend::domain::OfflineValidationError>(())
/// ```
#[derive(Debug, Clone)]
pub struct OfflineBundleDraft {
    /// Unique bundle identifier.
    pub id: Uuid,
    /// Optional owning user.
    pub owner_user_id: Option<UserId>,
    /// Device identifier used for sync scoping.
    pub device_id: String,
    /// Bundle type (`Route` or `Region`).
    pub kind: OfflineBundleKind,
    /// Route id for route-scoped bundles.
    pub route_id: Option<Uuid>,
    /// Region id for region-scoped bundles.
    pub region_id: Option<String>,
    /// Geographic bounds.
    pub bounds: BoundingBox,
    /// Tile zoom coverage.
    pub zoom_range: ZoomRange,
    /// Estimated manifest size in bytes.
    pub estimated_size_bytes: u64,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
    /// Download lifecycle status.
    pub status: OfflineBundleStatus,
    /// Status progress value in `[0.0, 1.0]`.
    pub progress: f32,
}

/// Offline bundle manifest metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct OfflineBundle {
    pub(super) id: Uuid,
    pub(super) owner_user_id: Option<UserId>,
    pub(super) device_id: String,
    pub(super) kind: OfflineBundleKind,
    pub(super) route_id: Option<Uuid>,
    pub(super) region_id: Option<String>,
    pub(super) bounds: BoundingBox,
    pub(super) zoom_range: ZoomRange,
    pub(super) estimated_size_bytes: u64,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
    pub(super) status: OfflineBundleStatus,
    pub(super) progress: f32,
}

impl OfflineBundle {
    /// Creates an offline bundle after validating draft invariants.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let draft = backend::domain::OfflineBundleDraft {
    /// #     id: uuid::Uuid::new_v4(),
    /// #     owner_user_id: None,
    /// #     device_id: "ios-phone".to_owned(),
    /// #     kind: backend::domain::OfflineBundleKind::Route,
    /// #     route_id: Some(uuid::Uuid::new_v4()),
    /// #     region_id: None,
    /// #     bounds: backend::domain::BoundingBox::new(-3.25, 55.92, -3.10, 56.01)?,
    /// #     zoom_range: backend::domain::ZoomRange::new(12, 16)?,
    /// #     estimated_size_bytes: 12_000_000,
    /// #     created_at: chrono::Utc::now(),
    /// #     updated_at: chrono::Utc::now(),
    /// #     status: backend::domain::OfflineBundleStatus::Queued,
    /// #     progress: 0.0,
    /// # };
    /// let bundle = backend::domain::OfflineBundle::new(draft)?;
    /// assert_eq!(bundle.status(), backend::domain::OfflineBundleStatus::Queued);
    /// Ok::<(), backend::domain::OfflineValidationError>(())
    /// ```
    pub fn new(draft: OfflineBundleDraft) -> Result<Self, OfflineValidationError> {
        Self::try_from(draft)
    }

    /// Returns the bundle identifier.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let bundle = sample_bundle()?;
    /// assert!(!bundle.id().is_nil());
    /// # Ok::<(), backend::domain::OfflineValidationError>(())
    /// ```
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Returns the optional owner id.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let bundle = sample_bundle()?;
    /// let _ = bundle.owner_user_id();
    /// # Ok::<(), backend::domain::OfflineValidationError>(())
    /// ```
    pub fn owner_user_id(&self) -> Option<&UserId> {
        self.owner_user_id.as_ref()
    }

    /// Returns the normalized device id.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let bundle = sample_bundle()?;
    /// assert!(!bundle.device_id().trim().is_empty());
    /// # Ok::<(), backend::domain::OfflineValidationError>(())
    /// ```
    pub fn device_id(&self) -> &str {
        self.device_id.as_str()
    }

    /// Returns the bundle kind.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let bundle = sample_bundle()?;
    /// let _ = bundle.kind();
    /// # Ok::<(), backend::domain::OfflineValidationError>(())
    /// ```
    pub fn kind(&self) -> OfflineBundleKind {
        self.kind
    }

    /// Returns the route id when available.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let bundle = sample_bundle()?;
    /// let _ = bundle.route_id();
    /// # Ok::<(), backend::domain::OfflineValidationError>(())
    /// ```
    pub fn route_id(&self) -> Option<Uuid> {
        self.route_id
    }

    /// Returns the region id when available.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let bundle = sample_bundle()?;
    /// let _ = bundle.region_id();
    /// # Ok::<(), backend::domain::OfflineValidationError>(())
    /// ```
    pub fn region_id(&self) -> Option<&str> {
        self.region_id.as_deref()
    }

    /// Returns geographic bounds metadata.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let bundle = sample_bundle()?;
    /// let [_, _, _, _] = bundle.bounds().as_array();
    /// # Ok::<(), backend::domain::OfflineValidationError>(())
    /// ```
    pub fn bounds(&self) -> BoundingBox {
        self.bounds
    }

    /// Returns the inclusive zoom range.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let bundle = sample_bundle()?;
    /// assert!(bundle.zoom_range().min_zoom() <= bundle.zoom_range().max_zoom());
    /// # Ok::<(), backend::domain::OfflineValidationError>(())
    /// ```
    pub fn zoom_range(&self) -> ZoomRange {
        self.zoom_range
    }

    /// Returns the estimated bundle size in bytes.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let bundle = sample_bundle()?;
    /// assert!(bundle.estimated_size_bytes() > 0);
    /// # Ok::<(), backend::domain::OfflineValidationError>(())
    /// ```
    pub fn estimated_size_bytes(&self) -> u64 {
        self.estimated_size_bytes
    }

    /// Returns the creation timestamp.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let bundle = sample_bundle()?;
    /// assert!(bundle.updated_at() >= bundle.created_at());
    /// # Ok::<(), backend::domain::OfflineValidationError>(())
    /// ```
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Returns the last-update timestamp.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let bundle = sample_bundle()?;
    /// assert!(bundle.updated_at() >= bundle.created_at());
    /// # Ok::<(), backend::domain::OfflineValidationError>(())
    /// ```
    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    /// Returns lifecycle status.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let bundle = sample_bundle()?;
    /// let _ = bundle.status();
    /// # Ok::<(), backend::domain::OfflineValidationError>(())
    /// ```
    pub fn status(&self) -> OfflineBundleStatus {
        self.status
    }

    /// Returns completion progress in `[0.0, 1.0]`.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// # let bundle = sample_bundle()?;
    /// assert!((0.0..=1.0).contains(&bundle.progress()));
    /// # Ok::<(), backend::domain::OfflineValidationError>(())
    /// ```
    pub fn progress(&self) -> f32 {
        self.progress
    }
}
