Feature: Session lifecycle hardening for user profile and interests
  The user profile and interest update endpoints must require an authenticated
  session and return trace identifiers when unauthenticated.

  Scenario: Unauthenticated profile request is rejected
    Given a running server with session middleware
    When the client requests the current user without a session
    Then the response is unauthorised with a trace id

  Scenario: Unauthenticated interests update is rejected
    Given a running server with session middleware
    When the client updates interests without a session
    Then the response is unauthorised with a trace id

  Scenario: Authenticated profile request uses the profile port
    Given a running server with session middleware
    And the client has an authenticated session
    When the client requests the current user profile
    Then the profile response includes the expected display name
    And the profile port was called with the authenticated user id

  Scenario: Authenticated interests update uses the interests port
    Given a running server with session middleware
    And the client has an authenticated session
    When the client updates interest selections
    Then the interests response includes the selected theme
    And the interests port was called with the authenticated user id and theme

  Scenario: Authenticated interests update validates interest theme ids
    Given a running server with session middleware
    And the client has an authenticated session
    When the client updates interests with an invalid interest theme id
    Then the response is a bad request with interest theme validation details
