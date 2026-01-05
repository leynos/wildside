//! Query implementation for the route annotations service.

use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::ports::{
    IdempotencyRepository, RouteAnnotationRepository, RouteAnnotationsQuery,
};
use crate::domain::{Error, RouteAnnotations, UserId};

use super::service::RouteAnnotationsService;

#[async_trait]
impl<R, I> RouteAnnotationsQuery for RouteAnnotationsService<R, I>
where
    R: RouteAnnotationRepository,
    I: IdempotencyRepository,
{
    async fn fetch_annotations(
        &self,
        route_id: Uuid,
        user_id: &UserId,
    ) -> Result<RouteAnnotations, Error> {
        let notes = self
            .annotations_repo
            .find_notes_by_route_and_user(&route_id, user_id)
            .await
            .map_err(Self::map_annotations_error)?;

        let progress = self
            .annotations_repo
            .find_progress(&route_id, user_id)
            .await
            .map_err(Self::map_annotations_error)?;

        Ok(RouteAnnotations {
            route_id,
            notes,
            progress,
        })
    }
}
