-- Revert baseline schema for roadmap item 3.1.1.

DROP TABLE IF EXISTS safety_presets;
DROP TABLE IF EXISTS safety_toggles;
DROP TABLE IF EXISTS badges;
DROP TABLE IF EXISTS tags;

DROP TABLE IF EXISTS community_picks;
DROP TABLE IF EXISTS trending_route_highlights;
DROP TABLE IF EXISTS route_collections;
DROP TABLE IF EXISTS route_summaries;
DROP TABLE IF EXISTS themes;
DROP TABLE IF EXISTS route_categories;

DROP TABLE IF EXISTS route_pois;
DROP TABLE IF EXISTS poi_interest_themes;
DROP INDEX IF EXISTS idx_pois_osm_tags_gin;
DROP INDEX IF EXISTS idx_pois_location_gist;
DROP TABLE IF EXISTS pois;
DROP TABLE IF EXISTS user_interest_themes;
DROP TABLE IF EXISTS interest_themes;

DROP INDEX IF EXISTS idx_routes_path_gist;

ALTER TABLE routes
    DROP COLUMN IF EXISTS generation_params,
    DROP COLUMN IF EXISTS path,
    ADD COLUMN IF NOT EXISTS request_id UUID NOT NULL DEFAULT gen_random_uuid(),
    ADD COLUMN IF NOT EXISTS plan_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ALTER COLUMN user_id SET NOT NULL;

CREATE INDEX IF NOT EXISTS idx_routes_request_id ON routes (request_id);

CREATE TRIGGER update_routes_updated_at
    BEFORE UPDATE ON routes
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
