-- Create user_preferences table for storing user preference settings.
--
-- This table stores interest themes, safety toggles, and display preferences
-- with optimistic concurrency via a revision column.

CREATE TABLE user_preferences (
    user_id UUID PRIMARY KEY REFERENCES users(id),
    interest_theme_ids UUID[] NOT NULL DEFAULT '{}',
    safety_toggle_ids UUID[] NOT NULL DEFAULT '{}',
    unit_system TEXT NOT NULL DEFAULT 'metric'
        CHECK (unit_system IN ('metric', 'imperial')),
    revision INTEGER NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for querying by last update time.
CREATE INDEX idx_user_preferences_updated_at ON user_preferences (updated_at);

-- Attach the updated_at trigger.
CREATE TRIGGER update_user_preferences_updated_at
    BEFORE UPDATE ON user_preferences
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
