-- Revert users table creation.
--
-- This migration drops all objects created in up.sql in reverse order.

DROP TRIGGER IF EXISTS update_users_updated_at ON users;
DROP FUNCTION IF EXISTS update_updated_at_column();
DROP INDEX IF EXISTS idx_users_display_name;
DROP TABLE IF EXISTS users;
