-- Revert offline bundle and walk session tables for roadmap 3.3.2.

DROP TRIGGER IF EXISTS update_walk_sessions_updated_at ON walk_sessions;
DROP INDEX IF EXISTS idx_walk_sessions_user_completed_ended_at_desc;
DROP TABLE IF EXISTS walk_sessions;

DROP TRIGGER IF EXISTS update_offline_bundles_updated_at ON offline_bundles;
DROP INDEX IF EXISTS idx_offline_bundles_anonymous_device_created_at;
DROP INDEX IF EXISTS idx_offline_bundles_owner_device_created_at;
DROP TABLE IF EXISTS offline_bundles;
