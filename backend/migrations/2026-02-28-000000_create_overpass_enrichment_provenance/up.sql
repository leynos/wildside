CREATE TABLE IF NOT EXISTS overpass_enrichment_provenance (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    source_url TEXT NOT NULL CHECK (char_length(source_url) > 0),
    imported_at TIMESTAMPTZ NOT NULL,
    bounds_min_lng DOUBLE PRECISION NOT NULL CHECK (bounds_min_lng >= -180.0 AND bounds_min_lng <= 180.0),
    bounds_min_lat DOUBLE PRECISION NOT NULL CHECK (bounds_min_lat >= -90.0 AND bounds_min_lat <= 90.0),
    bounds_max_lng DOUBLE PRECISION NOT NULL CHECK (bounds_max_lng >= -180.0 AND bounds_max_lng <= 180.0),
    bounds_max_lat DOUBLE PRECISION NOT NULL CHECK (bounds_max_lat >= -90.0 AND bounds_max_lat <= 90.0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT overpass_enrichment_provenance_bounds_order CHECK (
        bounds_min_lng <= bounds_max_lng
        AND bounds_min_lat <= bounds_max_lat
    )
);

CREATE INDEX IF NOT EXISTS idx_overpass_enrichment_provenance_imported_at
    ON overpass_enrichment_provenance (imported_at DESC, id DESC);
