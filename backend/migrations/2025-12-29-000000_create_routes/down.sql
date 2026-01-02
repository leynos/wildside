-- Revert routes table creation.
--
-- This migration drops all objects created in up.sql in reverse order.

DROP TRIGGER IF EXISTS update_routes_updated_at ON routes;
DROP INDEX IF EXISTS idx_routes_created_at;
DROP INDEX IF EXISTS idx_routes_request_id;
DROP INDEX IF EXISTS idx_routes_user_id;
DROP TABLE IF EXISTS routes;
