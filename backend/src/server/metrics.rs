//! Optional Prometheus metrics middleware wrapper.

use actix_service::{
    Service, ServiceExt as _, Transform,
    boxed::{self, BoxService},
};
use actix_web::body::BoxBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::middleware::Compat;
use actix_web_prom::PrometheusMetrics;
use futures_util::future::LocalBoxFuture;
use std::sync::Arc;

/// Conditional Prometheus metrics middleware layer.
///
/// Wraps a service with metrics collection when enabled, or transparently bypasses
/// metrics when disabled. Useful for optional observability without runtime overhead
/// when metrics are not required.
#[derive(Clone)]
pub(crate) enum MetricsLayer {
    /// Metrics collection is enabled; wraps the inner service with PrometheusMetrics.
    Enabled(Arc<PrometheusMetrics>),
    /// Metrics collection is disabled; the inner service is used without instrumentation.
    Disabled,
}

impl MetricsLayer {
    /// Construct a MetricsLayer from an optional PrometheusMetrics instance.
    ///
    /// Returns `Enabled` if metrics is Some, or `Disabled` if None.
    #[must_use]
    pub(crate) fn from_option(metrics: Option<PrometheusMetrics>) -> Self {
        match metrics {
            Some(metrics) => Self::Enabled(Arc::new(metrics)),
            None => Self::Disabled,
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for MetricsLayer
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    B: actix_web::body::MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = actix_web::Error;
    type InitError = ();
    type Transform = BoxService<ServiceRequest, ServiceResponse<BoxBody>, actix_web::Error>;
    type Future = LocalBoxFuture<'static, Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        match self.clone() {
            MetricsLayer::Enabled(metrics) => {
                let fut = Compat::new((*metrics).clone()).new_transform(service);
                Box::pin(async move {
                    let svc = fut.await?;
                    Ok(boxed::service(svc))
                })
            }
            MetricsLayer::Disabled => Box::pin(async move {
                let svc = service.map(|res: ServiceResponse<B>| res.map_into_boxed_body());
                Ok(boxed::service(svc))
            }),
        }
    }
}
