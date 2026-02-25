# OSM ingestion end-to-end runbook

This runbook documents how to run and verify the backend `ingest-osm` command
end to end.

## Overview

The backend ships an `ingest-osm` command-line interface (CLI) that ingests
OpenStreetMap `.osm.pbf` data into backend storage with launch geofence
filtering, provenance persistence, and deterministic reruns keyed by
`(geofence_id, input_digest)`.

Use this runbook for:

- execute a manual ingestion for a launch region;
- validate deterministic rerun behaviour;
- verify persisted provenance and POI writes;
- diagnose ingestion failures.

## Prerequisites

Before running ingestion:

- [ ] Ensure the command runs from the repository root.
- [ ] Ensure a reachable PostgreSQL instance exists and `DATABASE_URL` is set.
- [ ] Ensure the backend schema is current (including the
      `osm_ingestion_provenance` table).
- [ ] Ensure an input `.osm.pbf` file and canonical source URL are available.
- [ ] Ensure the geofence identifier and bounds are final for the run.

Example preflight commands:

```bash
git rev-parse --show-toplevel
echo "${DATABASE_URL:?DATABASE_URL must be set}"
psql "$DATABASE_URL" -c "\d osm_ingestion_provenance"
```

Expected `--geofence-bounds` format:

```plaintext
min_lng,min_lat,max_lng,max_lat
```

## Ingestion procedure

### 1. Set run inputs

Choose values for the run and export them:

```bash
export OSM_PBF_PATH="/absolute/path/to/region.osm.pbf"
export SOURCE_URL="https://download.geofabrik.de/europe/united-kingdom-latest.osm.pbf"
export GEOFENCE_ID="launch-edinburgh"
export GEOFENCE_BOUNDS="-3.35,55.85,-3.05,56.02"
```

### 2. Run ingestion

Run the backend CLI:

```bash
cargo run --manifest-path backend/Cargo.toml --bin ingest-osm -- \
  --osm-pbf "$OSM_PBF_PATH" \
  --source-url "$SOURCE_URL" \
  --geofence-id "$GEOFENCE_ID" \
  --geofence-bounds "$GEOFENCE_BOUNDS"
```

The command accepts `--database-url` explicitly. If omitted, it uses
`DATABASE_URL`.

### 3. Capture CLI output

The CLI prints one key-value line per field. A successful first run should
include:

```plaintext
status=Executed
source_url=...
geofence_id=...
input_digest=...
imported_at=...
geofence_bounds=...
raw_poi_count=...
persisted_poi_count=...
```

## Post-run verification

Verify provenance persistence:

```bash
psql "$DATABASE_URL" -c "
SELECT
  geofence_id,
  input_digest,
  source_url,
  raw_poi_count,
  filtered_poi_count,
  imported_at
FROM osm_ingestion_provenance
WHERE geofence_id = '$GEOFENCE_ID'
ORDER BY imported_at DESC
LIMIT 5;
"
```

Verify POI writes for the geofence bounds:

```bash
psql "$DATABASE_URL" -c "
SELECT COUNT(*) AS poi_count
FROM pois
WHERE split_part(trim(both '()' FROM location::text), ',', 1)::double precision
      BETWEEN -3.35 AND -3.05
  AND split_part(trim(both '()' FROM location::text), ',', 2)::double precision
      BETWEEN 55.85 AND 56.02;
"
```

If different bounds were used, replace the numeric predicates with the run
values.

## Deterministic rerun verification

Run the identical ingestion command a second time with the same:

- `--osm-pbf` file content;
- `--geofence-id`;
- `--geofence-bounds`.

Expected result:

- CLI prints `status=Replayed`.
- No duplicate provenance rows are created for the same
  `(geofence_id, input_digest)`.
- `persisted_poi_count` equals `0` for the replayed outcome.

Verification query:

```bash
psql "$DATABASE_URL" -c "
SELECT COUNT(*) AS provenance_rows
FROM osm_ingestion_provenance
WHERE geofence_id = '$GEOFENCE_ID'
  AND input_digest = (
    SELECT input_digest
    FROM osm_ingestion_provenance
    WHERE geofence_id = '$GEOFENCE_ID'
    ORDER BY imported_at DESC
    LIMIT 1
  );
"
```

Expected `provenance_rows` is `1`.

## Remediation and rollback

If a run used incorrect source metadata or geofence settings, correct the
inputs and rerun ingestion with the intended values.

If a bad provenance row must be removed, delete by both geofence and digest so
unrelated ingestion history remains unaffected:

```bash
psql "$DATABASE_URL" -c "
DELETE FROM osm_ingestion_provenance
WHERE geofence_id = '<geofence-id>'
  AND input_digest = '<input-digest>';
"
```

Only perform targeted deletes with change control approval.

## Troubleshooting

### `database URL missing: set --database-url or DATABASE_URL`

Set `DATABASE_URL` or pass `--database-url` explicitly.

### `geofence bounds must contain exactly four comma-separated numeric values`

Fix `--geofence-bounds` to match `min_lng,min_lat,max_lng,max_lat`.

### `ingest command failed: invalid request`

Validate:

- `source_url` is an absolute URL;
- `geofence_id` is non-empty;
- geofence bounds are finite and ordered (`min <= max`);
- input digest is computed from a real `.osm.pbf` file.

### `status=Replayed` on the first expected run

A prior run already created provenance for the same
`(geofence_id, input_digest)`. Confirm with:

```bash
psql "$DATABASE_URL" -c "
SELECT geofence_id, input_digest, imported_at
FROM osm_ingestion_provenance
WHERE geofence_id = '$GEOFENCE_ID'
ORDER BY imported_at DESC
LIMIT 10;
"
```

## Related documentation

- [Backend architecture: data seeding and enrichment workflow](../wildside-backend-architecture.md#data-seeding-and-enrichment-workflow)
- [Backend architecture: roadmap 3.4.1 design decision](../wildside-backend-architecture.md#driving-ports-services-and-queries)
- [ExecPlan: ingest-osm CLI implementation](../execplans/backend-3-4-1-ingest-osm-command-line.md)
