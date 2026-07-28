//! Prometheus metric assertions for the Apalis route queue test seam.
//!
//! These tests keep Prometheus text snapshots separate from the unit tests that
//! exercise provider and serialization behaviour through recording metrics.

use super::*;
use crate::outbound::metrics::PrometheusRouteQueueMetrics;
use insta::assert_snapshot;
use prometheus::Encoder;

#[tokio::test]
async fn concurrent_enqueue_with_metrics_records_correct_count() {
    let registry = prometheus::Registry::new();
    let metrics = PrometheusRouteQueueMetrics::new(&registry)
        .expect("route queue metrics should register with isolated registry");
    let queue = Arc::new(TestQueue::new(FakeQueueProvider::new(), Arc::new(metrics)));

    ensure_all_enqueues_succeed(spawn_enqueues(queue, 4))
        .await
        .expect("all concurrent enqueues should succeed");

    let encoded = encode_route_queue_metrics(&registry).expect("metrics should encode");
    let normalized = normalize_route_queue_metrics(&encoded).expect("metrics should normalize");
    assert_snapshot!(
        normalized,
        @r###"
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.0005"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.001"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.0025"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.005"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.01"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.025"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.05"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.1"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.25"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.5"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="1"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="+Inf"} <bucket_count>
route_queue_enqueue_latency_seconds_count{outcome="success"} 4
route_queue_enqueue_latency_seconds_sum{outcome="success"} <latency_sum>
route_queue_enqueue_total{outcome="success"} 4
"###
    );
}

#[tokio::test]
async fn apalis_queue_records_prometheus_enqueue_metrics() {
    let registry = prometheus::Registry::new();
    let metrics = PrometheusRouteQueueMetrics::new(&registry)
        .expect("route queue metrics should register with isolated registry");
    let queue = GenericApalisRouteQueue::new(FakeQueueProvider::new(), Arc::new(metrics));

    queue
        .enqueue(&TestPlan {
            name: "test-plan".to_string(),
        })
        .await
        .expect("enqueue should succeed with fake provider");

    let encoded = encode_route_queue_metrics(&registry).expect("metrics should encode");
    let normalized = normalize_route_queue_metrics(&encoded).expect("metrics should normalize");
    assert_snapshot!(
        normalized,
        @r###"
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.0005"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.001"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.0025"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.005"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.01"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.025"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.05"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.1"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.25"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.5"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="1"} <bucket_count>
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="+Inf"} <bucket_count>
route_queue_enqueue_latency_seconds_count{outcome="success"} 1
route_queue_enqueue_latency_seconds_sum{outcome="success"} <latency_sum>
route_queue_enqueue_total{outcome="success"} 1
"###
    );
}

fn encode_route_queue_metrics(registry: &prometheus::Registry) -> Result<String, String> {
    let mut buffer = Vec::new();
    prometheus::TextEncoder::new()
        .encode(&registry.gather(), &mut buffer)
        .map_err(|error| format!("failed to encode Prometheus metrics: {error}"))?;
    String::from_utf8(buffer).map_err(|error| format!("metrics text is not UTF-8: {error}"))
}

fn normalize_route_queue_metrics(text: &str) -> Result<String, String> {
    let mut lines = text
        .lines()
        .filter(|line| line.starts_with("route_queue_enqueue_"))
        .map(normalize_timing_sample)
        .collect::<Result<Vec<_>, _>>()?;
    lines.sort_by_key(|line| line.replace("le=\"+Inf\"", "le=\"z\""));
    Ok(lines.join("\n"))
}

fn normalize_timing_sample(line: &str) -> Result<String, String> {
    if line.starts_with("route_queue_enqueue_latency_seconds_bucket") {
        let (labels, _) = line
            .rsplit_once(' ')
            .ok_or_else(|| format!("metric sample has no value separator: {line}"))?;
        return Ok(format!("{labels} <bucket_count>"));
    }
    if line.starts_with("route_queue_enqueue_latency_seconds_sum") {
        let (labels, _) = line
            .rsplit_once(' ')
            .ok_or_else(|| format!("metric sample has no value separator: {line}"))?;
        return Ok(format!("{labels} <latency_sum>"));
    }
    Ok(line.to_string())
}
