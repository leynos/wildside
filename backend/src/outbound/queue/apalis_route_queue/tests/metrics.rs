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

    assert_all_enqueues_succeed(spawn_enqueues(queue, 4)).await;

    assert_snapshot!(
        normalize_route_queue_metrics(&encode_route_queue_metrics(&registry)),
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

    assert_snapshot!(
        normalize_route_queue_metrics(&encode_route_queue_metrics(&registry)),
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

fn encode_route_queue_metrics(registry: &prometheus::Registry) -> String {
    let mut buffer = Vec::new();
    if let Err(error) = prometheus::TextEncoder::new().encode(&registry.gather(), &mut buffer) {
        panic!("metrics should encode as Prometheus text: {error}");
    }
    match String::from_utf8(buffer) {
        Ok(text) => text,
        Err(error) => panic!("metrics text should be UTF-8: {error}"),
    }
}

fn normalize_route_queue_metrics(text: &str) -> String {
    let mut lines = text
        .lines()
        .filter(|line| line.starts_with("route_queue_enqueue_"))
        .map(normalize_timing_sample)
        .collect::<Vec<_>>();
    lines.sort_by_key(|line| line.replace("le=\"+Inf\"", "le=\"z\""));
    lines.join("\n")
}

fn normalize_timing_sample(line: &str) -> String {
    if line.starts_with("route_queue_enqueue_latency_seconds_bucket") {
        return format!(
            "{} <bucket_count>",
            line.rsplit_once(' ').expect("sample").0
        );
    }
    if line.starts_with("route_queue_enqueue_latency_seconds_sum") {
        return format!("{} <latency_sum>", line.rsplit_once(' ').expect("sample").0);
    }
    line.to_string()
}
