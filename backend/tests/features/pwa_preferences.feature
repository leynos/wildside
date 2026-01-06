Feature: PWA preferences endpoints
  The PWA needs session-authenticated preferences endpoints with consistent
  error envelopes and idempotency support.

  Scenario: Authenticated preferences fetch returns preferences
    Given a running server with session middleware
    And the client has an authenticated session
    And the preferences query returns default preferences
    When the client requests preferences
    Then the response is ok
    And the preferences response includes the expected unit system
    And the preferences query was called with the authenticated user id

  Scenario: Preferences update validates unit system
    Given a running server with session middleware
    And the client has an authenticated session
    When the client updates preferences with an invalid unit system
    Then the response is a bad request with unit system details

  Scenario: Preferences update uses the idempotency key
    Given a running server with session middleware
    And the client has an authenticated session
    And the preferences command returns updated preferences
    When the client updates preferences with an idempotency key
    Then the response is ok
    And the preferences response includes revision 2
    And the preferences command captures the idempotency key

  Scenario: Preferences update rejects stale revision
    Given a running server with session middleware
    And the client has an authenticated session
    And the preferences command returns a revision mismatch
    When the client updates preferences with expected revision 1
    Then the response is a conflict error with revision details

  Scenario: Preferences update rejects idempotency conflict
    Given a running server with session middleware
    And the client has an authenticated session
    And the preferences command returns an idempotency conflict
    When the client updates preferences with an idempotency key
    Then the response is a conflict error with idempotency message

  Scenario: Preferences update replays cached response
    Given a running server with session middleware
    And the client has an authenticated session
    And the preferences command returns a replayed response
    When the client updates preferences with an idempotency key
    Then the response is ok
    And the preferences response includes replayed true
