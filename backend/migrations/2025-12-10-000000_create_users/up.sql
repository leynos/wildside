-- Create users table for UserRepository persistence.
--
-- This table stores registered users with their display names and audit
-- timestamps. The schema mirrors the domain User entity constraints.

CREATE TABLE users (
    id UUID PRIMARY KEY,
    display_name VARCHAR(32) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for potential future queries by display name.
CREATE INDEX idx_users_display_name ON users (display_name);

-- Trigger function to auto-update the updated_at timestamp on row modification.
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Attach the trigger to the users table.
CREATE TRIGGER update_users_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
