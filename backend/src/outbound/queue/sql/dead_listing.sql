SELECT id, job_type, attempts,
       metadata->>'wildside.retry.dead_reason' AS dead_reason,
       metadata->>'wildside.retry.requeue_count' AS requeue_count,
       CASE WHEN metadata->>'wildside.retry.dead_reason'
                     IN ('fatal', 'exhausted')
                 AND jsonb_typeof(last_result->'Err') = 'string'
            THEN last_result->>'Err' END AS last_error,
       done_at AS dead_at
FROM apalis.jobs
WHERE status = 'Killed'
   OR (status = 'Failed'
       AND (max_attempts IS NULL OR attempts >= max_attempts))
ORDER BY done_at DESC NULLS LAST, id DESC
LIMIT $1 OFFSET $2
