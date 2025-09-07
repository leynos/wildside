//! Tracing middleware attaching a request-scoped trace identifier.
//!
//! Each incoming request receives a UUID `trace_id` stored in task-local
//! context for correlation across logs and error responses.

use std::task::{Context, Poll};

use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::{Error, HttpMessage};
use futures_util::future::{ready, LocalBoxFuture, Ready};
use tokio::task_local;
use tracing::info_span;
use uuid::Uuid;

/// Task-local storage for the current request's trace identifier.
///
/// # Examples
/// ```
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

task_local! {
    static TRACE_ID: String;
}

/// Retrieve the trace identifier for the current task if set.
///
/// # Examples
/// ```
/// use backend::middleware::trace::current_trace_id;
///
/// if let Some(id) = current_trace_id() {
///     println!("{}", id);
/// }
/// ```
pub fn current_trace_id() -> Option<String> {
    TRACE_ID.try_with(|id| id.clone()).ok()
}

/// Borrow the current trace identifier without allocation (internal use).
#[allow(dead_code)]
pub(crate) fn current_trace_id_ref() -> Option<&'static str> {
    TRACE_ID
        .try_with(|id| unsafe {
            // SAFETY: TRACE_ID lives for the duration of the task scope.
            std::mem::transmute::<&str, &'static str>(id.as_str())
        })
        .ok()
}

/// Tracing middleware attaching a request-scoped UUID and
/// adding a `Trace-Id` header to every response.
///
/// Call [`current_trace_id`] in a handler to read the identifier.
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
        let span =
            info_span!("request", trace_id = %trace_id, method = %req.method(), path = %req.path());
        let fut = self.service.call(req);

        Box::pin(TRACE_ID.scope(trace_id.clone(), async move {
            let _enter = span.enter();
            let mut res = fut.await?;
            res.response_mut().headers_mut().insert(
                HeaderName::from_static("trace-id"),
                HeaderValue::from_str(&trace_id).expect("valid Trace-Id header value"),
            );
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
            web::get().to(|| async { ApiResult::<HttpResponse>::Err(Error::internal("boom")) }),
        ))
        .await;
        let req = test::TestRequest::get().uri("/").to_request();
        let res = test::call_service(&app, req).await;
        assert!(res.headers().contains_key("trace-id"));
        let body: Error = test::read_body_json(res).await;
        assert!(body.trace_id.is_some());
    }
}
