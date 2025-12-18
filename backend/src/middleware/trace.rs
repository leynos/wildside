//! Tracing middleware attaching a request-scoped trace identifier.
//!
//! Each incoming request receives a UUID `trace_id` stored in task-local
//! storage for correlation across logs and error responses.
//!
//! The [`TraceId`] type itself lives in [`crate::domain::trace_id`]. This
//! module provides the Actix Web middleware that generates and propagates
//! trace identifiers for HTTP requests.

use std::task::{Context, Poll};

use crate::domain::{TraceId, TRACE_ID_HEADER};
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::Error;
use futures_util::future::{ready, LocalBoxFuture, Ready};
use tracing::error;

/// Tracing middleware attaching a request-scoped UUID and
/// adding a `Trace-Id` header to every response.
///
/// Handlers can read the trace ID via the `TraceId::current` helper.
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
                        .insert(HeaderName::from_static(TRACE_ID_HEADER), value);
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
    use actix_web::{web, App, HttpResponse};

    #[actix_web::test]
    async fn adds_trace_id_header() {
        let app = actix_web::test::init_service(
            App::new()
                .wrap(Trace)
                .route("/", web::get().to(|| async { HttpResponse::Ok().finish() })),
        )
        .await;
        let req = actix_web::test::TestRequest::get().uri("/").to_request();
        let res = actix_web::test::call_service(&app, req).await;
        assert!(res.headers().contains_key(TRACE_ID_HEADER));
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
        let app = actix_web::test::init_service(
            App::new().wrap(Trace).route("/", web::get().to(handler)),
        )
        .await;
        let req = actix_web::test::TestRequest::get().uri("/").to_request();
        let res = actix_web::test::call_service(&app, req).await;
        let trace_id = res
            .headers()
            .get(TRACE_ID_HEADER)
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
        let body = actix_web::test::read_body(res).await;
        let body = std::str::from_utf8(&body).expect("utf8 body");
        assert_eq!(trace_id, body);
    }

    #[actix_web::test]
    async fn propagates_trace_id_in_error() {
        use crate::domain::Error;

        let (res, trace_id) = test_trace_with_handler(|| async move {
            // Error::internal captures the scoped TraceId automatically.
            Result::<HttpResponse, Error>::Err(Error::internal("boom"))
        })
        .await;
        let body: Error = actix_web::test::read_body_json(res).await;
        assert_eq!(body.trace_id(), Some(trace_id.as_str()));
    }
}
