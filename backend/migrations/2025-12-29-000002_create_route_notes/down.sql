-- Revert route_notes table creation.
--
-- This migration drops all objects created in up.sql in reverse order.

DROP TRIGGER IF EXISTS update_route_notes_updated_at ON route_notes;
DROP INDEX IF EXISTS idx_route_notes_updated_at;
DROP INDEX IF EXISTS idx_route_notes_route_user;
DROP TABLE IF EXISTS route_notes;
