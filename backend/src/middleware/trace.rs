//! Tracing middleware attaching a request-scoped trace identifier.
//!
//! Each incoming request receives a UUID `trace_id` stored in task-local
//! storage for correlation across logs and error responses.

use std::task::{Context, Poll};

use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::Error;
use futures_util::future::{ready, LocalBoxFuture, Ready};
use tokio::task_local;
use tracing::error;
use uuid::Uuid;

task_local! {
    static TRACE_ID: TraceId;
}

/// Per-request trace identifier exposed via task-local storage.
///
/// # Examples
/// ```
/// use backend::middleware::trace::TraceId;
///
/// async fn handler() {
///     if let Some(id) = TraceId::current() {
///         println!("trace id: {}", id.as_str());
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct TraceId(pub String);

impl TraceId {
    fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Returns the current trace identifier if one is in scope.
    pub fn current() -> Option<Self> {
        TRACE_ID.try_with(|id| id.clone()).ok()
    }

    /// Borrow the trace identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the identifier, yielding its owned string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

/// Tracing middleware attaching a request-scoped UUID and
/// adding a `Trace-Id` header to every response.
///
/// Handlers can read the trace ID via [`TraceId::current`].
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
        let trace_id = TraceId::generate();
        let fut = self.service.call(req);
        Box::pin(TRACE_ID.scope(trace_id.clone(), async move {
            let mut res = fut.await?;
            match HeaderValue::from_str(trace_id.as_str()) {
                Ok(value) => {
                    res.response_mut()
                        .headers_mut()
                        .insert(HeaderName::from_static("trace-id"), value);
                }
                Err(error) => {
                    error!(
                        %error,
                        trace_id = %trace_id.as_str(),
                        "failed to encode trace identifier header"
                    );
                }
            }
            Ok(res)
        }))
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

        let app = test::init_service(App::new().wrap(Trace).route(
            "/",
            web::get().to(|| async move {
                let id = TraceId::current()
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
