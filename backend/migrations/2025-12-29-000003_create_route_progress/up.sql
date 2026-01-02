-- Create route_progress table for tracking user progress on routes.
--
-- This table stores which stops a user has visited on a route,
-- with optimistic concurrency via a revision column.

CREATE TABLE route_progress (
    route_id UUID NOT NULL REFERENCES routes(id),
    user_id UUID NOT NULL REFERENCES users(id),
    visited_stop_ids UUID[] NOT NULL DEFAULT '{}',
    revision INTEGER NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (route_id, user_id)
);

-- Index for querying by last update time.
CREATE INDEX idx_route_progress_updated_at ON route_progress (updated_at);

-- Attach the updated_at trigger.
CREATE TRIGGER update_route_progress_updated_at
    BEFORE UPDATE ON route_progress
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
