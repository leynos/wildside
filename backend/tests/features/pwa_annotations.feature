Feature: PWA annotations endpoints
  The PWA needs session-authenticated endpoints for notes and progress with
  idempotency support and consistent error envelopes.

  Scenario: Route annotations fetch returns notes and progress
    Given a running server with session middleware
    And the client has an authenticated session
    And the annotations query returns a note and progress
    When the client requests annotations for the route
    Then the response is ok
    And the annotations response includes the note and progress
    And the annotations query was called with the authenticated user id

  Scenario: Note upsert uses the idempotency key
    Given a running server with session middleware
    And the client has an authenticated session
    And the annotations command returns an upserted note
    When the client upserts a note with an idempotency key
    Then the response is ok
    And the note response includes the note id
    And the note command captures the idempotency key

  Scenario: Progress update surfaces conflicts
    Given a running server with session middleware
    And the client has an authenticated session
    And the progress update is configured to conflict
    When the client updates progress with a valid payload
    Then the response is a conflict error
