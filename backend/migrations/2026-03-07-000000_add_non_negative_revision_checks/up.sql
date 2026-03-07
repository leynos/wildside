-- Enforce non-negative optimistic concurrency revisions for persisted records.

ALTER TABLE user_preferences
    ADD CONSTRAINT chk_user_preferences_revision_non_negative
    CHECK (revision >= 0);

ALTER TABLE route_notes
    ADD CONSTRAINT chk_route_notes_revision_non_negative
    CHECK (revision >= 0);
