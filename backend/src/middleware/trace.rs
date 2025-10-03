//! Tracing middleware attaching a request-scoped trace identifier.
//!
//! Each incoming request receives a UUID `trace_id` stored in request extensions
//! for correlation across logs and error responses.

use std::task::{Context, Poll};

use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::{Error, HttpMessage};
use futures_util::future::{ready, LocalBoxFuture, Ready};
use tracing::error;
use uuid::Uuid;

/// Per-request trace identifier stored in request extensions.
///
/// # Examples
/// ```ignore
/// use actix_web::HttpRequest;
/// use backend::middleware::trace::TraceId;
///
/// fn handler(req: HttpRequest) {
///     if let Some(id) = req.extensions().get::<TraceId>() {
///         println!("trace id: {}", id.0);
///     }
/// }
/// ```
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TraceId(pub String);

/// Tracing middleware attaching a request-scoped UUID and
/// adding a `Trace-Id` header to every response.
///
/// Handlers can read the trace ID via `req.extensions().get::<TraceId>()`.
///
/// # Examples
/// ```
/// use actix_web::App;
/// use backend::Trace;
///
/// let app = App::new().wrap(Trace);
/// ```
#[derive(Clone)]
pub struct Trace;

impl<S, B> Transform<S, ServiceRequest> for Trace
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = TraceMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(TraceMiddleware { service }))
    }
}

/// Service wrapper produced by [`Trace`].
///
/// Applications should not use this type directly.
///
/// # Examples
/// ```
/// use actix_web::App;
/// use backend::Trace;
///
/// let _app = App::new().wrap(Trace);
/// ```
pub struct TraceMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for TraceMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let trace_id = Uuid::new_v4().to_string();
        req.extensions_mut().insert(TraceId(trace_id.clone()));
        let fut = self.service.call(req);
        Box::pin(async move {
            let mut res = fut.await?;
            if let Err(error) = HeaderValue::from_str(&trace_id).map(|value| {
                res.response_mut()
                    .headers_mut()
                    .insert(HeaderName::from_static("trace-id"), value);
            }) {
                error!(%error, trace_id = %trace_id, "failed to encode trace identifier header");
            }
            Ok(res)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App, HttpResponse};

    #[actix_web::test]
    async fn adds_trace_id_header() {
        let app = test::init_service(
            App::new()
                .wrap(Trace)
                .route("/", web::get().to(|| async { HttpResponse::Ok().finish() })),
        )
        .await;
        let req = test::TestRequest::get().uri("/").to_request();
        let res = test::call_service(&app, req).await;
        assert!(res.headers().contains_key("trace-id"));
    }

    #[actix_web::test]
    async fn propagates_trace_id_in_error() {
        use crate::models::{ApiResult, Error};
        use actix_web::HttpRequest;

        let app = test::init_service(App::new().wrap(Trace).route(
            "/",
            web::get().to(|req: HttpRequest| async move {
                let id = req
                    .extensions()
                    .get::<TraceId>()
                    .cloned()
                    .ok_or_else(|| Error::internal("trace id missing"))?
                    .0;
                ApiResult::<HttpResponse>::Err(Error::internal("boom").with_trace_id(id))
            }),
        ))
        .await;
        let req = test::TestRequest::get().uri("/").to_request();
        let res = test::call_service(&app, req).await;
        assert!(res.headers().contains_key("trace-id"));
        let body: Error = test::read_body_json(res).await;
        assert!(body.trace_id.is_some());
    }
}
