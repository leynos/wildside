Feature: Catalogue read endpoints
  The PWA needs session-authenticated endpoints for catalogue and
  descriptor snapshots with cache headers and generated_at metadata.

  Scenario: Explore catalogue returns snapshot with cache headers
    Given a running server with session middleware
    And the client has an authenticated session
    When the client requests the explore catalogue
    Then the response is ok
    And the response includes a generated_at timestamp
    And the explore response includes empty arrays for all collections

  Scenario: Descriptors returns snapshot with cache headers
    Given a running server with session middleware
    And the client has an authenticated session
    When the client requests the descriptors
    Then the response is ok
    And the response includes a generated_at timestamp
    And the descriptors response includes empty arrays for all registries

  Scenario: Explore catalogue requires authentication
    Given a running server with session middleware
    When the client requests the explore catalogue without a session
    Then the response is unauthorised

  Scenario: Descriptors requires authentication
    Given a running server with session middleware
    When the client requests the descriptors without a session
    Then the response is unauthorised

  Scenario: Explore catalogue surfaces connection error as 503
    Given a running server with session middleware
    And the client has an authenticated session
    And the catalogue repository returns a connection error
    When the client requests the explore catalogue
    Then the response is service unavailable

  Scenario: Descriptors surfaces connection error as 503
    Given a running server with session middleware
    And the client has an authenticated session
    And the descriptor repository returns a connection error
    When the client requests the descriptors
    Then the response is service unavailable
