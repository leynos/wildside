-- Create route_notes table for storing user annotations on routes.
--
-- This table stores notes that users can attach to routes or specific POIs
-- within routes, with optimistic concurrency via a revision column.

CREATE TABLE route_notes (
    id UUID PRIMARY KEY,
    route_id UUID NOT NULL REFERENCES routes(id),
    poi_id UUID,
    user_id UUID NOT NULL REFERENCES users(id),
    body TEXT NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for querying notes by route and user.
CREATE INDEX idx_route_notes_route_user ON route_notes (route_id, user_id);

-- Index for querying by last update time.
CREATE INDEX idx_route_notes_updated_at ON route_notes (updated_at);

-- Attach the updated_at trigger.
CREATE TRIGGER update_route_notes_updated_at
    BEFORE UPDATE ON route_notes
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
