//! Route annotations HTTP handlers.
//!
//! ```text
//! GET /api/v1/routes/{route_id}/annotations
//! POST /api/v1/routes/{route_id}/notes
//! PUT /api/v1/routes/{route_id}/progress
//! ```

#[path = "annotations_dto.rs"]
mod annotations_dto;

use std::future::Future;

use actix_web::{FromRequest, HttpRequest, dev::Payload, get, post, put, web};
use futures_util::future::LocalBoxFuture;
use uuid::Uuid;

use crate::domain::ports::{
    UpdateProgressRequest, UpdateProgressResponse, UpsertNoteRequest, UpsertNoteResponse,
};
use crate::domain::{Error, IdempotencyKey, UserId};
use crate::inbound::http::ApiResult;
use crate::inbound::http::idempotency::{extract_idempotency_key, map_idempotency_key_error};
use crate::inbound::http::schemas::ErrorSchema;
use crate::inbound::http::session::SessionContext;
use crate::inbound::http::state::HttpState;

use annotations_dto::{
    NoteRequest, ParsedNoteRequest, ParsedProgressRequest, ProgressRequest,
    RouteAnnotationsResponse, RouteNoteResponse, RoutePath, RouteProgressResponse,
    parse_note_request, parse_progress_request, parse_route_id,
};

struct RouteMutationContext {
    user_id: UserId,
    route_id: Uuid,
    idempotency_key: Option<IdempotencyKey>,
}

struct RouteMutationHeaders {
    session: SessionContext,
    idempotency_key: Option<IdempotencyKey>,
}

impl RouteMutationHeaders {
    fn into_context(self, route_id: Uuid) -> Result<RouteMutationContext, Error> {
        let user_id = self.session.require_user_id()?;
        Ok(RouteMutationContext {
            user_id,
            route_id,
            idempotency_key: self.idempotency_key,
        })
    }
}

impl FromRequest for RouteMutationHeaders {
    type Error = actix_web::Error;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let headers = req.headers().clone();
        let session_fut = SessionContext::from_request(req, payload);
        Box::pin(async move {
            let session = session_fut.await?;
            let idempotency_key = extract_idempotency_key(&headers)
                .map_err(map_idempotency_key_error)
                .map_err(actix_web::Error::from)?;
            Ok(Self {
                session,
                idempotency_key,
            })
        })
    }
}

struct RouteMutationSpec<Payload, Parsed, Req, DomainRes, Response, ParseFn, BuildFn, CallFn, MapFn>
{
    payload: web::Json<Payload>,
    parse: ParseFn,
    build_request: BuildFn,
    call: CallFn,
    map_response: MapFn,
    _parsed: std::marker::PhantomData<(Parsed, Req, DomainRes, Response)>,
}

async fn handle_route_mutation<
    Payload,
    Parsed,
    Req,
    DomainRes,
    Response,
    ParseFn,
    BuildFn,
    CallFn,
    MapFn,
    Fut,
>(
    headers: RouteMutationHeaders,
    route_id: Uuid,
    spec: RouteMutationSpec<
        Payload,
        Parsed,
        Req,
        DomainRes,
        Response,
        ParseFn,
        BuildFn,
        CallFn,
        MapFn,
    >,
) -> ApiResult<web::Json<Response>>
where
    ParseFn: FnOnce(Payload) -> Result<Parsed, Error>,
    BuildFn: FnOnce(RouteMutationContext, Parsed) -> Req,
    CallFn: FnOnce(Req) -> Fut,
    MapFn: FnOnce(DomainRes) -> Response,
    Fut: Future<Output = Result<DomainRes, Error>>,
{
    let RouteMutationSpec {
        payload,
        parse,
        build_request,
        call,
        map_response,
        _parsed,
    } = spec;
    let context = headers.into_context(route_id)?;
    let parsed = parse(payload.into_inner())?;
    let response = call(build_request(context, parsed)).await?;
    Ok(web::Json(map_response(response)))
}

macro_rules! route_mutation_handler {
    (
        $(#[$meta:meta])*
        $name:ident,
        response = $response_ty:ty,
        payload = $payload_ty:ty,
        parse = $parse:expr,
        build = $build:expr,
        call_method = $call_method:ident,
        map = $map:expr $(,)?
    ) => {
        $(#[$meta])*
        pub async fn $name(
            state: web::Data<HttpState>,
            headers: RouteMutationHeaders,
            path: web::Path<RoutePath>,
            payload: web::Json<$payload_ty>,
        ) -> ApiResult<web::Json<$response_ty>> {
            let command = state.route_annotations.clone();
            let route_id = parse_route_id(path.into_inner())?;
            handle_route_mutation(
                headers,
                route_id,
                RouteMutationSpec {
                    payload,
                    parse: $parse,
                    build_request: $build,
                    call: move |request| {
                        let command = command.clone();
                        async move { command.$call_method(request).await }
                    },
                    map_response: $map,
                    _parsed: std::marker::PhantomData,
                },
            )
            .await
        }
    };
}

/// Fetch notes and progress for a route.
#[utoipa::path(
    get,
    path = "/api/v1/routes/{route_id}/annotations",
    params(
        ("route_id" = String, Path, description = "Route identifier")
    ),
    responses(
        (status = 200, description = "Route annotations", body = RouteAnnotationsResponse),
        (status = 400, description = "Invalid request", body = ErrorSchema),
        (status = 401, description = "Unauthorised", body = ErrorSchema),
        (status = 404, description = "Not found", body = ErrorSchema),
        (status = 500, description = "Internal server error", body = ErrorSchema)
    ),
    tags = ["routes"],
    operation_id = "getRouteAnnotations"
)]
#[get("/routes/{route_id}/annotations")]
pub async fn get_annotations(
    state: web::Data<HttpState>,
    session: SessionContext,
    path: web::Path<RoutePath>,
) -> ApiResult<web::Json<RouteAnnotationsResponse>> {
    let user_id = session.require_user_id()?;
    let route_id = parse_route_id(path.into_inner())?;
    let annotations = state
        .route_annotations_query
        .fetch_annotations(route_id, &user_id)
        .await?;
    Ok(web::Json(RouteAnnotationsResponse::from(annotations)))
}

route_mutation_handler!(
    /// Create or update a note for the route.
    #[utoipa::path(
        post,
        path = "/api/v1/routes/{route_id}/notes",
        request_body = NoteRequest,
        params(
            ("route_id" = String, Path, description = "Route identifier"),
            ("Idempotency-Key" = Option<String>, Header, description = "UUID for idempotent requests")
        ),
        responses(
            (status = 200, description = "Upserted note", body = RouteNoteResponse),
            (status = 400, description = "Invalid request", body = ErrorSchema),
            (status = 401, description = "Unauthorised", body = ErrorSchema),
            (status = 409, description = "Conflict", body = ErrorSchema),
            (status = 503, description = "Service unavailable", body = ErrorSchema)
        ),
        tags = ["routes"],
        operation_id = "upsertRouteNote"
    )]
    #[post("/routes/{route_id}/notes")]
    upsert_note,
    response = RouteNoteResponse,
    payload = NoteRequest,
    parse = parse_note_request,
    build = |context: RouteMutationContext, parsed: ParsedNoteRequest| UpsertNoteRequest {
        note_id: parsed.note_id,
        route_id: context.route_id,
        poi_id: parsed.poi_id,
        user_id: context.user_id,
        body: parsed.body,
        expected_revision: parsed.expected_revision,
        idempotency_key: context.idempotency_key,
    },
    call_method = upsert_note,
    map = |response: UpsertNoteResponse| RouteNoteResponse::from(response.note),
);

route_mutation_handler!(
    /// Update route progress for the authenticated user.
    #[utoipa::path(
        put,
        path = "/api/v1/routes/{route_id}/progress",
        request_body = ProgressRequest,
        params(
            ("route_id" = String, Path, description = "Route identifier"),
            ("Idempotency-Key" = Option<String>, Header, description = "UUID for idempotent requests")
        ),
        responses(
            (status = 200, description = "Updated progress", body = RouteProgressResponse),
            (status = 400, description = "Invalid request", body = ErrorSchema),
            (status = 401, description = "Unauthorised", body = ErrorSchema),
            (status = 409, description = "Conflict", body = ErrorSchema),
            (status = 503, description = "Service unavailable", body = ErrorSchema)
        ),
        tags = ["routes"],
        operation_id = "updateRouteProgress"
    )]
    #[put("/routes/{route_id}/progress")]
    update_progress,
    response = RouteProgressResponse,
    payload = ProgressRequest,
    parse = parse_progress_request,
    build = |context: RouteMutationContext, parsed: ParsedProgressRequest| UpdateProgressRequest {
        route_id: context.route_id,
        user_id: context.user_id,
        visited_stop_ids: parsed.visited_stop_ids,
        expected_revision: parsed.expected_revision,
        idempotency_key: context.idempotency_key,
    },
    call_method = update_progress,
    map = |response: UpdateProgressResponse| RouteProgressResponse::from(response.progress),
);

#[cfg(test)]
#[path = "annotations_tests.rs"]
mod tests;
