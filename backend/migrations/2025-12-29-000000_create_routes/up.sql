-- Create routes table for storing generated route plans.
--
-- This table is a prerequisite for route_notes and route_progress tables
-- which have foreign key constraints to routes.

CREATE TABLE routes (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id),
    request_id UUID NOT NULL,
    plan_snapshot JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for querying routes by user.
CREATE INDEX idx_routes_user_id ON routes (user_id);

-- Index for finding routes by request ID.
CREATE INDEX idx_routes_request_id ON routes (request_id);

-- Index for ordering by creation time.
CREATE INDEX idx_routes_created_at ON routes (created_at);

-- Attach the updated_at trigger (function created in create_users migration).
CREATE TRIGGER update_routes_updated_at
    BEFORE UPDATE ON routes
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
