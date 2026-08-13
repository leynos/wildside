//! Public HTTP-boundary coverage for the Overpass adapter.

use std::{
    error::Error as StdError,
    net::TcpListener,
    sync::{Arc, Mutex},
    time::Duration,
};

use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer,
    dev::ServerHandle,
    http::{Method, StatusCode},
    web,
};
use backend::domain::BoundingBox;
use backend::domain::ports::{EnrichmentRequest, EnrichmentSource, EnrichmentSourceError};
use backend::outbound::overpass::{OverpassHttpIdentity, OverpassHttpSource};
use url::Url;
use uuid::Uuid;

type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

fn request(tags: Vec<&str>) -> EnrichmentRequest {
    let bounding_box = match BoundingBox::new(-3.30, 55.90, -3.10, 56.00) {
        Ok(bounding_box) => bounding_box,
        Err(error) => panic!("fixture bounding box should be valid: {error}"),
    };
    EnrichmentRequest {
        job_id: Uuid::new_v4(),
        bounding_box,
        tags: tags.into_iter().map(str::to_owned).collect(),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct RecordedRequest {
    method: Method,
    user_agent: Option<String>,
    contact: Option<String>,
    accept: Option<String>,
    content_type: Option<String>,
    body: String,
}

#[derive(Clone)]
struct LocalOverpassState {
    status: StatusCode,
    body: &'static str,
    recorded_request: Arc<Mutex<Option<RecordedRequest>>>,
}

async fn respond_from_local_overpass(
    request: HttpRequest,
    body: web::Bytes,
    state: web::Data<LocalOverpassState>,
) -> HttpResponse {
    let recorded_request = RecordedRequest {
        method: request.method().clone(),
        user_agent: request
            .headers()
            .get("User-Agent")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
        contact: request
            .headers()
            .get("Contact")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
        accept: request
            .headers()
            .get("Accept")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
        content_type: request
            .headers()
            .get("Content-Type")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned),
        body: String::from_utf8_lossy(&body).into_owned(),
    };
    let mut stored_request = match state.recorded_request.lock() {
        Ok(stored_request) => stored_request,
        Err(error) => panic!("local Overpass request lock poisoned: {error}"),
    };
    assert!(
        stored_request.is_none(),
        "local server should receive one request"
    );
    *stored_request = Some(recorded_request);

    HttpResponse::build(state.status).body(state.body)
}

async fn start_local_overpass(
    status: StatusCode,
    body: &'static str,
) -> TestResult<(Url, Arc<Mutex<Option<RecordedRequest>>>, ServerHandle)> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let address = listener.local_addr()?;
    let recorded_request = Arc::new(Mutex::new(None));
    let state = LocalOverpassState {
        status,
        body,
        recorded_request: Arc::clone(&recorded_request),
    };
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .default_service(web::to(respond_from_local_overpass))
    })
    .listen(listener)?
    .disable_signals()
    .run();
    let handle = server.handle();
    actix_web::rt::spawn(server);

    Ok((
        Url::parse(&format!("http://{address}"))?,
        recorded_request,
        handle,
    ))
}

fn form_data(body: &str) -> Option<String> {
    url::form_urlencoded::parse(body.as_bytes()).find_map(|(key, value)| {
        if key == "data" {
            Some(value.into_owned())
        } else {
            None
        }
    })
}

#[actix_web::test]
async fn fetch_pois_posts_identity_and_bbox_query_to_the_endpoint() -> TestResult {
    let response_body = r#"{
        "elements": [{
            "type": "node",
            "id": 101,
            "lat": 55.91,
            "lon": -3.21,
            "tags": { "amenity": "cafe" }
        }]
    }"#;
    let (endpoint, recorded_request, server) =
        start_local_overpass(StatusCode::OK, response_body).await?;
    let source = OverpassHttpSource::with_identity(
        endpoint.clone(),
        Duration::from_secs(2),
        OverpassHttpIdentity {
            user_agent: "wildside-overpass-test/1.0".to_owned(),
            contact: "tests@wildside.invalid".to_owned(),
            query_timeout_seconds: 37,
        },
    )?;

    let response = source
        .fetch_pois(&request(vec!["amenity", "name=coffee"]))
        .await?;
    server.stop(true).await;

    assert_eq!(response.pois.len(), 1, "valid Overpass JSON should decode");
    assert_eq!(response.pois[0].element_id, 101);
    assert_eq!(response.transfer_bytes, response_body.len() as u64);
    assert_eq!(response.source_url, endpoint.to_string());

    let recorded_request = recorded_request
        .lock()
        .expect("local Overpass request lock")
        .take()
        .expect("fetch_pois should POST to the configured endpoint");
    assert_eq!(recorded_request.method, Method::POST);
    assert_eq!(
        recorded_request.user_agent.as_deref(),
        Some("wildside-overpass-test/1.0")
    );
    assert_eq!(
        recorded_request.contact.as_deref(),
        Some("tests@wildside.invalid")
    );
    assert_eq!(recorded_request.accept.as_deref(), Some("application/json"));
    assert!(
        recorded_request
            .content_type
            .as_deref()
            .is_some_and(|value| value.starts_with("application/x-www-form-urlencoded")),
        "fetch_pois should submit an HTML form body",
    );
    let query = form_data(&recorded_request.body).expect("form body should contain query data");
    assert!(query.starts_with("[out:json][timeout:37];"));
    assert!(
        query.contains("node[\"amenity\"](55.9,-3.3,56,-3.1);"),
        "the query should use the BoundingBox south,west,north,east order",
    );
    assert!(
        query.contains("relation[\"name\"=\"coffee\"](55.9,-3.3,56,-3.1);"),
        "the query should include every requested tag and Overpass element type",
    );
    Ok(())
}

#[actix_web::test]
async fn fetch_pois_maps_non_success_http_responses() -> TestResult {
    let (endpoint, _recorded_request, server) =
        start_local_overpass(StatusCode::TOO_MANY_REQUESTS, "slow down").await?;
    let source = OverpassHttpSource::new(endpoint, Duration::from_secs(2))?;

    let error = source
        .fetch_pois(&request(vec!["amenity"]))
        .await
        .expect_err("429 responses should be returned as domain errors");
    server.stop(true).await;

    assert!(
        matches!(
            error,
            EnrichmentSourceError::RateLimited { ref message }
                if message == "status 429: slow down"
        ),
        "a non-success HTTP response should preserve its status mapping",
    );
    Ok(())
}
