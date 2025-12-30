-- Revert route_progress table creation.
--
-- This migration drops all objects created in up.sql in reverse order.

DROP TRIGGER IF EXISTS update_route_progress_updated_at ON route_progress;
DROP INDEX IF EXISTS idx_route_progress_updated_at;
DROP TABLE IF EXISTS route_progress;
