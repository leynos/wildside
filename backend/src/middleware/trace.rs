//! Tracing middleware attaching a request-scoped trace identifier.
//!
//! Each incoming request receives a UUID `trace_id` stored in task-local
//! storage for correlation across logs and error responses.
//!
//! Tokio task-local variables are not inherited across spawned tasks. Use
//! [`TraceId::scope`] when spawning new tasks or moving work onto blocking
//! threads to ensure the active trace identifier propagates correctly.

use std::task::{Context, Poll};

use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::Error;
use futures_util::future::{ready, LocalBoxFuture, Ready};
use std::future::Future;
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
///         println!("trace id: {}", id);
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceId(pub(crate) Uuid);

impl TraceId {
    #[rustfmt::skip]
    fn generate() -> Self { Self(Uuid::new_v4()) }

    /// Returns the current trace identifier if one is in scope.
    #[rustfmt::skip]
    pub fn current() -> Option<Self> { TRACE_ID.try_with(|id| *id).ok() }

    /// Execute the provided future with the supplied trace identifier in scope.
    ///
    /// # Examples
    /// ```
    /// use backend::middleware::trace::TraceId;
    ///
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// let trace_id: TraceId = "00000000-0000-0000-0000-000000000000"
    ///     .parse()
    ///     .expect("valid UUID");
    /// let observed = TraceId::scope(trace_id, async move { TraceId::current() }).await;
    /// assert_eq!(observed, Some(trace_id));
    /// # });
    /// ```
    pub async fn scope<Fut>(trace_id: TraceId, fut: Fut) -> Fut::Output
    where
        Fut: Future,
    {
        TRACE_ID.scope(trace_id, fut).await
    }
}

impl std::fmt::Display for TraceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for TraceId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
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
        let header_value = trace_id.to_string();
        let fut = self.service.call(req);
        Box::pin(TraceId::scope(trace_id, async move {
            let mut res = fut.await?;
            match HeaderValue::from_str(&header_value) {
                Ok(value) => {
                    res.response_mut()
                        .headers_mut()
                        .insert(HeaderName::from_static("trace-id"), value);
                }
                Err(error) => {
                    error!(
                        %error,
                        trace_id = %trace_id,
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
    #[tokio::test]
    async fn trace_id_generate_produces_uuid() {
        let trace_id = TraceId::generate();
        let parsed = Uuid::parse_str(&trace_id.to_string()).expect("valid UUID");
        assert_eq!(parsed.to_string(), trace_id.to_string());
    }

    #[tokio::test]
    async fn trace_id_current_reflects_scope() {
        let expected = TraceId::generate();
        let observed = TraceId::scope(expected, async move { TraceId::current() }).await;
        assert_eq!(observed, Some(expected));
    }

    #[tokio::test]
    async fn trace_id_current_is_none_out_of_scope() {
        assert!(TraceId::current().is_none());
    }

    #[tokio::test]
    async fn trace_id_from_str_round_trips() {
        let uuid = Uuid::nil();
        let trace_id: TraceId = uuid.to_string().parse().expect("parse uuid");
        assert_eq!(trace_id.to_string(), uuid.to_string());
    }

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

    async fn test_trace_with_handler<F, Fut, Res>(
        handler: F,
    ) -> (
        actix_web::dev::ServiceResponse<actix_web::body::BoxBody>,
        String,
    )
    where
        F: Fn() -> Fut + Clone + 'static,
        Fut: std::future::Future<Output = Res> + 'static,
        Res: actix_web::Responder + 'static,
    {
        let app =
            test::init_service(App::new().wrap(Trace).route("/", web::get().to(handler))).await;
        let req = test::TestRequest::get().uri("/").to_request();
        let res = test::call_service(&app, req).await;
        let trace_id = res
            .headers()
            .get("trace-id")
            .expect("trace id header")
            .to_str()
            .expect("header is ascii")
            .to_owned();
        (res, trace_id)
    }

    #[actix_web::test]
    async fn exposes_trace_id_in_handler() {
        let (res, trace_id) = test_trace_with_handler(|| async move {
            let id = TraceId::current().expect("trace id in scope");
            HttpResponse::Ok().body(id.to_string())
        })
        .await;
        let body = test::read_body(res).await;
        let body = std::str::from_utf8(&body).expect("utf8 body");
        assert_eq!(trace_id, body);
    }

    #[actix_web::test]
    async fn propagates_trace_id_in_error() {
        use crate::models::{ApiResult, Error};

        let (res, trace_id) = test_trace_with_handler(|| async move {
            // Error::internal captures the scoped TraceId automatically.
            ApiResult::<HttpResponse>::Err(Error::internal("boom"))
        })
        .await;
        let body: Error = test::read_body_json(res).await;
        assert_eq!(body.trace_id.as_deref(), Some(trace_id.as_str()));
    }
}
