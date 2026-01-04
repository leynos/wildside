//! Driving port for route annotations queries.
//!
//! Inbound adapters (HTTP handlers) use this port to fetch notes and progress
//! for a route without importing outbound persistence concerns.

use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::{Error, RouteAnnotations, UserId};

/// Domain use-case port for fetching route annotations.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait RouteAnnotationsQuery: Send + Sync {
    /// Fetch notes and progress for a route and user.
    ///
    /// Note counts are expected to remain bounded by route POIs; pagination can
    /// be added if this assumption changes.
    async fn fetch_annotations(
        &self,
        route_id: Uuid,
        user_id: &UserId,
    ) -> Result<RouteAnnotations, Error>;
}

/// Fixture query returning empty annotations.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureRouteAnnotationsQuery;

#[async_trait]
impl RouteAnnotationsQuery for FixtureRouteAnnotationsQuery {
    async fn fetch_annotations(
        &self,
        route_id: Uuid,
        _user_id: &UserId,
    ) -> Result<RouteAnnotations, Error> {
        Ok(RouteAnnotations {
            route_id,
            notes: Vec::new(),
            progress: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fixture_query_returns_empty_annotations() {
        let query = FixtureRouteAnnotationsQuery;
        let route_id = Uuid::new_v4();
        let user_id = UserId::random();

        let annotations = query
            .fetch_annotations(route_id, &user_id)
            .await
            .expect("annotations fetched");

        assert_eq!(annotations.route_id, route_id);
        assert!(annotations.notes.is_empty());
        assert!(annotations.progress.is_none());
    }
}
