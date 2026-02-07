-- Baseline schema for data platform foundation (roadmap 3.1.1).
--
-- Materializes core spatial tables plus catalogue/descriptor read models and
-- preserves existing user state tables. This migration also aligns the routes
-- table with the backend architecture schema.

CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- ---------------------------------------------------------------------------
-- Align routes table with architecture schema
-- ---------------------------------------------------------------------------

DROP TRIGGER IF EXISTS update_routes_updated_at ON routes;
DROP INDEX IF EXISTS idx_routes_request_id;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_name = 'routes'
          AND column_name = 'plan_snapshot'
    )
    AND EXISTS (SELECT 1 FROM routes LIMIT 1) THEN
        RAISE EXCEPTION
            'schema baseline migration requires an empty routes table before removing plan_snapshot';
    END IF;
END $$;

ALTER TABLE routes
    DROP COLUMN IF EXISTS request_id,
    DROP COLUMN IF EXISTS plan_snapshot,
    DROP COLUMN IF EXISTS updated_at,
    ALTER COLUMN user_id DROP NOT NULL,
    ADD COLUMN IF NOT EXISTS path PATH NOT NULL DEFAULT '((0,0),(0,0))'::path,
    ADD COLUMN IF NOT EXISTS generation_params JSONB NOT NULL DEFAULT '{}'::jsonb;

-- ---------------------------------------------------------------------------
-- Core catalogue + spatial entities
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS interest_themes (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT
);

CREATE TABLE IF NOT EXISTS user_interest_themes (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    theme_id UUID NOT NULL REFERENCES interest_themes(id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, theme_id)
);

CREATE TABLE IF NOT EXISTS pois (
    element_type TEXT NOT NULL,
    id BIGINT NOT NULL,
    location POINT NOT NULL,
    osm_tags JSONB NOT NULL DEFAULT '{}'::jsonb,
    narrative TEXT,
    popularity_score REAL NOT NULL DEFAULT 0,
    PRIMARY KEY (element_type, id)
);

CREATE INDEX IF NOT EXISTS idx_pois_location_gist ON pois USING GIST (location);
CREATE INDEX IF NOT EXISTS idx_pois_osm_tags_gin ON pois USING GIN (osm_tags);

CREATE TABLE IF NOT EXISTS poi_interest_themes (
    poi_element_type TEXT NOT NULL,
    poi_id BIGINT NOT NULL,
    theme_id UUID NOT NULL REFERENCES interest_themes(id) ON DELETE CASCADE,
    PRIMARY KEY (poi_element_type, poi_id, theme_id),
    FOREIGN KEY (poi_element_type, poi_id)
        REFERENCES pois(element_type, id)
        ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS route_pois (
    route_id UUID NOT NULL REFERENCES routes(id) ON DELETE CASCADE,
    poi_element_type TEXT NOT NULL,
    poi_id BIGINT NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    PRIMARY KEY (route_id, poi_element_type, poi_id),
    FOREIGN KEY (poi_element_type, poi_id)
        REFERENCES pois(element_type, id)
        ON DELETE CASCADE,
    CONSTRAINT route_pois_route_position_unique UNIQUE (route_id, position)
);

-- ---------------------------------------------------------------------------
-- Catalogue read models
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS route_categories (
    id UUID PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    icon_key TEXT NOT NULL,
    localizations JSONB NOT NULL DEFAULT '{}'::jsonb,
    route_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS themes (
    id UUID PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    icon_key TEXT NOT NULL,
    localizations JSONB NOT NULL DEFAULT '{}'::jsonb,
    image JSONB NOT NULL DEFAULT '{}'::jsonb,
    walk_count INTEGER NOT NULL DEFAULT 0,
    distance_range_metres INTEGER[] NOT NULL DEFAULT ARRAY[0, 0],
    rating REAL NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS route_summaries (
    id UUID PRIMARY KEY,
    route_id UUID NOT NULL UNIQUE REFERENCES routes(id) ON DELETE CASCADE,
    category_id UUID NOT NULL REFERENCES route_categories(id),
    theme_id UUID NOT NULL REFERENCES themes(id),
    slug TEXT UNIQUE,
    localizations JSONB NOT NULL DEFAULT '{}'::jsonb,
    hero_image JSONB NOT NULL DEFAULT '{}'::jsonb,
    distance_metres INTEGER NOT NULL DEFAULT 0,
    duration_seconds INTEGER NOT NULL DEFAULT 0,
    rating REAL NOT NULL DEFAULT 0,
    badge_ids UUID[] NOT NULL DEFAULT '{}',
    difficulty TEXT NOT NULL DEFAULT 'easy',
    interest_theme_ids UUID[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS route_collections (
    id UUID PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    icon_key TEXT NOT NULL,
    localizations JSONB NOT NULL DEFAULT '{}'::jsonb,
    lead_image JSONB NOT NULL DEFAULT '{}'::jsonb,
    map_preview JSONB NOT NULL DEFAULT '{}'::jsonb,
    distance_range_metres INTEGER[] NOT NULL DEFAULT ARRAY[0, 0],
    duration_range_seconds INTEGER[] NOT NULL DEFAULT ARRAY[0, 0],
    difficulty TEXT NOT NULL DEFAULT 'easy',
    route_ids UUID[] NOT NULL DEFAULT '{}'
);

CREATE TABLE IF NOT EXISTS trending_route_highlights (
    id UUID PRIMARY KEY,
    route_summary_id UUID NOT NULL REFERENCES route_summaries(id) ON DELETE CASCADE,
    trend_delta TEXT NOT NULL DEFAULT '',
    subtitle_localizations JSONB NOT NULL DEFAULT '{}'::jsonb,
    highlighted_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS community_picks (
    id UUID PRIMARY KEY,
    route_summary_id UUID REFERENCES route_summaries(id),
    user_id UUID REFERENCES users(id),
    localizations JSONB NOT NULL DEFAULT '{}'::jsonb,
    curator_display_name TEXT NOT NULL,
    curator_avatar JSONB NOT NULL DEFAULT '{}'::jsonb,
    rating REAL NOT NULL DEFAULT 0,
    distance_metres INTEGER NOT NULL DEFAULT 0,
    duration_seconds INTEGER NOT NULL DEFAULT 0,
    saves INTEGER NOT NULL DEFAULT 0,
    picked_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ---------------------------------------------------------------------------
-- Descriptor registries
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS tags (
    id UUID PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    icon_key TEXT NOT NULL,
    localizations JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE IF NOT EXISTS badges (
    id UUID PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    icon_key TEXT NOT NULL,
    localizations JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE IF NOT EXISTS safety_toggles (
    id UUID PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    icon_key TEXT NOT NULL,
    localizations JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE IF NOT EXISTS safety_presets (
    id UUID PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    icon_key TEXT NOT NULL,
    localizations JSONB NOT NULL DEFAULT '{}'::jsonb,
    safety_toggle_ids UUID[] NOT NULL DEFAULT '{}'
);
