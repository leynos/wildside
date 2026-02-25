-- Roadmap 3.4.1: ingestion provenance and deterministic rerun key persistence.

CREATE TABLE IF NOT EXISTS osm_ingestion_provenance (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    geofence_id TEXT NOT NULL,
    source_url TEXT NOT NULL,
    input_digest TEXT NOT NULL,
    imported_at TIMESTAMPTZ NOT NULL,
    bounds_min_lng DOUBLE PRECISION NOT NULL,
    bounds_min_lat DOUBLE PRECISION NOT NULL,
    bounds_max_lng DOUBLE PRECISION NOT NULL,
    bounds_max_lat DOUBLE PRECISION NOT NULL,
    raw_poi_count BIGINT NOT NULL CHECK (raw_poi_count >= 0),
    filtered_poi_count BIGINT NOT NULL CHECK (filtered_poi_count >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT osm_ingestion_provenance_bounds_valid CHECK (
        bounds_min_lng >= -180 AND bounds_min_lng <= 180
        AND bounds_max_lng >= -180 AND bounds_max_lng <= 180
        AND bounds_min_lat >= -90 AND bounds_min_lat <= 90
        AND bounds_max_lat >= -90 AND bounds_max_lat <= 90
        AND bounds_min_lng <= bounds_max_lng
        AND bounds_min_lat <= bounds_max_lat
    ),
    CONSTRAINT osm_ingestion_provenance_rerun_unique UNIQUE (geofence_id, input_digest)
);

CREATE INDEX IF NOT EXISTS idx_osm_ingestion_provenance_geofence_imported_at
    ON osm_ingestion_provenance (geofence_id, imported_at DESC);
