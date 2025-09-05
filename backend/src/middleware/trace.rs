//! Tracing middleware attaching a request-scoped trace identifier.
//!
//! Each incoming request receives a UUID `trace_id` stored in task-local
//! context for correlation across logs and error responses.

use std::task::{Context, Poll};

use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{Error, HttpMessage};
use futures_util::future::{ready, LocalBoxFuture, Ready};
use tokio::task_local;
use tracing::info_span;
use uuid::Uuid;

// Task-local storage for the current request's trace identifier.
task_local! {
    static TRACE_ID: String;
}

/// Retrieve the trace identifier for the current task if set.
pub fn current_trace_id() -> Option<String> {
    TRACE_ID.try_with(|id| id.clone()).ok()
}

/// Middleware initialiser.
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
        req.extensions_mut().insert(trace_id.clone());
        let span = info_span!("request", trace_id = %trace_id, method = %req.method(), path = %req.path());
        let fut = self.service.call(req);

        Box::pin(TRACE_ID.scope(trace_id, async move {
            let _enter = span.enter();
            let res = fut.await?;
            Ok(res)
        }))
    }
}
