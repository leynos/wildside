//! Handler-level route submission request forwarding coverage.

use super::*;
use crate::domain::UserId;
use crate::domain::ports::{
    RouteLocation, RoutePreferences, RouteSubmissionRequest, RouteSubmissionResponse,
    RouteSubmissionService,
};
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Clone, Default)]
struct RecordingRouteSubmissionService(Arc<Mutex<Option<RouteSubmissionRequest>>>);

#[async_trait::async_trait]
impl RouteSubmissionService for RecordingRouteSubmissionService {
    async fn submit(
        &self,
        request: RouteSubmissionRequest,
    ) -> Result<RouteSubmissionResponse, crate::domain::Error> {
        let mut captured = match self.0.lock() {
            Ok(captured) => captured,
            Err(error) => panic!("recording service lock poisoned: {error}"),
        };
        assert!(
            captured.is_none(),
            "route submission should be recorded once"
        );
        *captured = Some(request);
        Ok(RouteSubmissionResponse::accepted(Uuid::nil()))
    }
}

#[actix_web::test]
async fn submit_route_forwards_every_typed_field_to_submission_service() -> TestResult {
    let recording_service = RecordingRouteSubmissionService::default();
    let app = actix_test::init_service(test_app(Arc::new(recording_service.clone()))).await;
    let cookie = login_and_get_cookie(&app).await?;
    let idempotency_key = "550e8400-e29b-41d4-a716-446655440000";

    let request = actix_test::TestRequest::post()
        .uri("/api/v1/routes")
        .cookie(cookie)
        .insert_header((IDEMPOTENCY_KEY_HEADER, idempotency_key))
        .set_json(json!({
            "origin": "saved:home",
            "destination": { "lat": 48.8566, "lng": 2.3522 },
            "preferences": {
                "mode": "walking",
                "themes": ["heritage", "water"],
                "themeIds": ["theme-1"],
                "interestThemeIds": ["interest-1"],
                "avoid": ["tunnels"],
                "avoidStairs": true
            }
        }))
        .to_request();

    let response = actix_test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let captured = recording_service
        .0
        .lock()
        .expect("recording service lock")
        .take()
        .expect("submission service should capture the request");
    assert_eq!(
        captured.user_id,
        UserId::new("123e4567-e89b-12d3-a456-426614174000")
            .expect("fixture user id should be valid")
    );
    assert_eq!(
        captured
            .idempotency_key
            .as_ref()
            .expect("idempotency key should be forwarded")
            .as_ref(),
        idempotency_key
    );
    assert_eq!(
        captured.payload.origin,
        RouteLocation::Identifier("saved:home".to_owned())
    );
    let destination = match captured.payload.destination {
        RouteLocation::Coordinates(coordinates) => coordinates,
        RouteLocation::Identifier(identifier) => {
            panic!("destination should be coordinates, got identifier {identifier}")
        }
    };
    assert_eq!(destination.lat(), 48.8566);
    assert_eq!(destination.lng(), 2.3522);
    assert_eq!(
        captured.payload.preferences,
        Some(RoutePreferences {
            mode: Some("walking".to_owned()),
            themes: Some(vec!["heritage".to_owned(), "water".to_owned()]),
            theme_ids: Some(vec!["theme-1".to_owned()]),
            interest_theme_ids: Some(vec!["interest-1".to_owned()]),
            avoid: Some(vec!["tunnels".to_owned()]),
            avoid_stairs: Some(true),
        })
    );
    Ok(())
}
