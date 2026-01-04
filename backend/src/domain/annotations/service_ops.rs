//! Command helpers for the route annotations service.
//!
//! Splits the service implementation into smaller units to keep files within
//! the size guidelines.

use crate::domain::ports::{IdempotencyRepository, RouteAnnotationRepository};
use crate::domain::{Error, RouteNote, RouteNoteContent, RouteProgress};

use super::service::RouteAnnotationsService;
use crate::domain::ports::{DeleteNoteRequest, UpdateProgressRequest, UpsertNoteRequest};

impl<R, I> RouteAnnotationsService<R, I>
where
    R: RouteAnnotationRepository,
    I: IdempotencyRepository,
{
    pub(super) async fn perform_upsert_note(
        &self,
        request: &UpsertNoteRequest,
    ) -> Result<RouteNote, Error> {
        let existing = self
            .annotations_repo
            .find_note_by_id(&request.note_id)
            .await
            .map_err(Self::map_annotations_error)?;

        if let Some(note) = &existing {
            if note.user_id != request.user_id {
                return Err(Error::forbidden("not authorised to update this note"));
            }
            if note.route_id != request.route_id {
                return Err(Error::conflict("note does not belong to this route"));
            }
        }

        let note = match (existing, request.expected_revision) {
            (None, None) => RouteNote::new(
                request.note_id,
                request.route_id,
                request.user_id.clone(),
                RouteNoteContent {
                    body: request.body.clone(),
                    poi_id: request.poi_id,
                },
            ),
            (None, Some(expected)) => {
                return Err(Self::revision_conflict(Some(expected), 0));
            }
            (Some(existing), None) => {
                return Err(Self::revision_conflict(None, existing.revision));
            }
            (Some(existing), Some(expected)) => {
                if existing.revision != expected {
                    return Err(Self::revision_conflict(Some(expected), existing.revision));
                }
                let existing_id = existing.id;
                let existing_route_id = existing.route_id;
                let existing_user_id = existing.user_id.clone();
                let existing_poi_id = existing.poi_id;
                let existing_created_at = existing.created_at;

                let mut builder =
                    RouteNote::builder(existing_id, existing_route_id, existing_user_id)
                        .body(request.body.clone())
                        .created_at(existing_created_at)
                        .updated_at(chrono::Utc::now())
                        .revision(expected + 1);

                if let Some(poi_id) = request.poi_id.or(existing_poi_id) {
                    builder = builder.poi_id(poi_id);
                }

                builder.build()
            }
        };

        self.annotations_repo
            .save_note(&note, request.expected_revision)
            .await
            .map_err(Self::map_annotations_error)?;

        Ok(note)
    }

    pub(super) async fn perform_update_progress(
        &self,
        request: &UpdateProgressRequest,
    ) -> Result<RouteProgress, Error> {
        let existing = self
            .annotations_repo
            .find_progress(&request.route_id, &request.user_id)
            .await
            .map_err(Self::map_annotations_error)?;

        let progress = match (existing, request.expected_revision) {
            (None, None) => RouteProgress::builder(request.route_id, request.user_id.clone())
                .visited_stop_ids(request.visited_stop_ids.clone())
                .revision(1)
                .build(),
            (None, Some(expected)) => {
                return Err(Self::revision_conflict(Some(expected), 0));
            }
            (Some(existing), None) => {
                return Err(Self::revision_conflict(None, existing.revision));
            }
            (Some(existing), Some(expected)) => {
                if existing.revision != expected {
                    return Err(Self::revision_conflict(Some(expected), existing.revision));
                }
                RouteProgress::builder(request.route_id, request.user_id.clone())
                    .visited_stop_ids(request.visited_stop_ids.clone())
                    .updated_at(chrono::Utc::now())
                    .revision(expected + 1)
                    .build()
            }
        };

        self.annotations_repo
            .save_progress(&progress, request.expected_revision)
            .await
            .map_err(Self::map_annotations_error)?;

        Ok(progress)
    }

    pub(super) async fn perform_delete_note(
        &self,
        request: &DeleteNoteRequest,
    ) -> Result<bool, Error> {
        if let Some(note) = self
            .annotations_repo
            .find_note_by_id(&request.note_id)
            .await
            .map_err(Self::map_annotations_error)?
        {
            if note.user_id != request.user_id {
                return Err(Error::forbidden("not authorised to delete this note"));
            }
        } else {
            return Ok(false);
        }

        self.annotations_repo
            .delete_note(&request.note_id)
            .await
            .map_err(Self::map_annotations_error)
    }
}
