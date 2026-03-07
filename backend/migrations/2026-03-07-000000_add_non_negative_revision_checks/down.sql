ALTER TABLE route_notes
    DROP CONSTRAINT chk_route_notes_revision_non_negative;

ALTER TABLE user_preferences
    DROP CONSTRAINT chk_user_preferences_revision_non_negative;
