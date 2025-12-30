-- Revert user_preferences table creation.
--
-- This migration drops all objects created in up.sql in reverse order.

DROP TRIGGER IF EXISTS update_user_preferences_updated_at ON user_preferences;
DROP INDEX IF EXISTS idx_user_preferences_updated_at;
DROP TABLE IF EXISTS user_preferences;
