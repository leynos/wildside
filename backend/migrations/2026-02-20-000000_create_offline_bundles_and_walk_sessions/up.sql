-- Create offline bundle and walk session tables for roadmap 3.3.2.

CREATE TABLE offline_bundles (
    id UUID PRIMARY KEY,
    owner_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    device_id TEXT NOT NULL CHECK (BTRIM(device_id) <> ''),
    kind TEXT NOT NULL CHECK (kind IN ('route', 'region')),
    route_id UUID REFERENCES routes(id) ON DELETE CASCADE,
    region_id TEXT,
    bounds DOUBLE PRECISION[] NOT NULL,
    min_zoom INTEGER NOT NULL CHECK (min_zoom BETWEEN 0 AND 255),
    max_zoom INTEGER NOT NULL CHECK (max_zoom BETWEEN 0 AND 255),
    estimated_size_bytes BIGINT NOT NULL CHECK (estimated_size_bytes >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    status TEXT NOT NULL CHECK (status IN ('queued', 'downloading', 'complete', 'failed')),
    progress REAL NOT NULL CHECK (progress >= 0.0 AND progress <= 1.0),
    CONSTRAINT offline_bundles_bounds_valid CHECK (
        cardinality(bounds) = 4
        AND bounds[1] BETWEEN -180.0 AND 180.0
        AND bounds[2] BETWEEN -90.0 AND 90.0
        AND bounds[3] BETWEEN -180.0 AND 180.0
        AND bounds[4] BETWEEN -90.0 AND 90.0
        AND bounds[1] <= bounds[3]
        AND bounds[2] <= bounds[4]
    ),
    CONSTRAINT offline_bundles_zoom_order_valid CHECK (min_zoom <= max_zoom),
    CONSTRAINT offline_bundles_kind_reference_valid CHECK (
        (kind = 'route' AND route_id IS NOT NULL AND region_id IS NULL)
        OR (
            kind = 'region'
            AND route_id IS NULL
            AND NULLIF(BTRIM(region_id), '') IS NOT NULL
        )
    ),
    CONSTRAINT offline_bundles_status_progress_valid CHECK (
        (status = 'queued' AND progress = 0.0)
        OR (status = 'downloading' AND progress > 0.0 AND progress < 1.0)
        OR (status = 'complete' AND progress = 1.0)
        OR (status = 'failed')
    ),
    CONSTRAINT offline_bundles_updated_after_created CHECK (updated_at >= created_at)
);

-- Supports owner/device listing ordered by creation time.
CREATE INDEX idx_offline_bundles_owner_device_created_at
    ON offline_bundles (owner_user_id, device_id, created_at, id);

-- Supports anonymous device listing ordered by creation time.
CREATE INDEX idx_offline_bundles_anonymous_device_created_at
    ON offline_bundles (device_id, created_at, id)
    WHERE owner_user_id IS NULL;

CREATE TRIGGER update_offline_bundles_updated_at
    BEFORE UPDATE ON offline_bundles
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TABLE walk_sessions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    route_id UUID NOT NULL REFERENCES routes(id) ON DELETE CASCADE,
    started_at TIMESTAMPTZ NOT NULL,
    ended_at TIMESTAMPTZ,
    primary_stats JSONB NOT NULL DEFAULT '[]'::jsonb
        CHECK (jsonb_typeof(primary_stats) = 'array'),
    secondary_stats JSONB NOT NULL DEFAULT '[]'::jsonb
        CHECK (jsonb_typeof(secondary_stats) = 'array'),
    highlighted_poi_ids UUID[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT walk_sessions_ended_after_started CHECK (
        ended_at IS NULL OR ended_at >= started_at
    ),
    CONSTRAINT walk_sessions_updated_after_created CHECK (updated_at >= created_at)
);

-- Supports completion summaries by user ordered by latest end time.
CREATE INDEX idx_walk_sessions_user_completed_ended_at_desc
    ON walk_sessions (user_id, ended_at DESC, id)
    WHERE ended_at IS NOT NULL;

CREATE TRIGGER update_walk_sessions_updated_at
    BEFORE UPDATE ON walk_sessions
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
