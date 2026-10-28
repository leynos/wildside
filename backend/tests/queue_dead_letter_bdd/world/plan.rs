//! Query-plan canary for the bounded dead-job listing.

use serde_json::Value;

use super::{QUEUE_NAME, QueueDeadLetterWorld};

const PENDING_ROW_COUNT: i32 = 5_000;
const DEAD_ROW_COUNT: i32 = 50;
const DEAD_LISTING_SQL: &str = include_str!("../../../src/outbound/queue/sql/dead_listing.sql");

impl QueueDeadLetterWorld {
    pub fn assert_listing_plan_avoids_sequential_scan(&self) {
        let seed = uuid::Uuid::new_v4().to_string();
        self.runtime
            .block_on(
                sqlx::query(
                    r#"INSERT INTO apalis.jobs
                    (id, job_type, job, status, attempts, max_attempts, run_at,
                     done_at, metadata)
                 SELECT $1 || '-pending-' || series::text, $2,
                        convert_to(jsonb_build_object(
                            'v', 'v1', 'probeId', $1 || '-pending-' || series::text
                        )::text, 'UTF8'),
                        'Pending', 0, 3, now(), NULL, '{}'::jsonb
                 FROM generate_series(1, $3) AS series"#,
                )
                .bind(&seed)
                .bind(QUEUE_NAME)
                .bind(PENDING_ROW_COUNT)
                .execute(&self.pool),
            )
            .expect("seed pending rows for dead-list plan");
        self.runtime
            .block_on(
                sqlx::query(
                    r#"INSERT INTO apalis.jobs
                    (id, job_type, job, status, attempts, max_attempts, run_at,
                     done_at, metadata)
                 SELECT $1 || '-dead-' || series::text, $2,
                        convert_to(jsonb_build_object(
                            'v', 'v1', 'probeId', $1 || '-dead-' || series::text
                        )::text, 'UTF8'),
                        CASE WHEN series % 2 = 0 THEN 'Killed' ELSE 'Failed' END,
                        3, 3, now(), now() - (series * interval '1 second'),
                        '{"wildside.retry.dead_reason":"exhausted"}'::jsonb
                 FROM generate_series(1, $3) AS series"#,
                )
                .bind(&seed)
                .bind(QUEUE_NAME)
                .bind(DEAD_ROW_COUNT)
                .execute(&self.pool),
            )
            .expect("seed dead rows for dead-list plan");
        self.runtime
            .block_on(sqlx::query("ANALYZE apalis.jobs").execute(&self.pool))
            .expect("collect statistics for dead-list plan");

        let plan = self.explain_dead_listing();
        let sequential_scan = contains_sequential_scan(&plan);
        println!(
            "dead-letter EXPLAIN ({PENDING_ROW_COUNT} pending, {DEAD_ROW_COUNT} dead): sequential_scan={sequential_scan}; {plan}"
        );
        assert!(
            !sequential_scan,
            "dead-letter listing must not scan the seeded Apalis table: {plan}"
        );
    }

    fn explain_dead_listing(&self) -> Value {
        let query = format!("EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {DEAD_LISTING_SQL}");
        self.runtime
            .block_on(
                sqlx::query_scalar(&query)
                    .bind(100_i64)
                    .bind(0_i64)
                    .fetch_one(&self.pool),
            )
            .expect("explain bounded dead listing")
    }
}

fn contains_sequential_scan(plan: &Value) -> bool {
    match plan {
        Value::Array(values) => values.iter().any(contains_sequential_scan),
        Value::Object(fields) => {
            fields.get("Node Type").and_then(Value::as_str) == Some("Seq Scan")
                || fields.values().any(contains_sequential_scan)
        }
        _ => false,
    }
}
