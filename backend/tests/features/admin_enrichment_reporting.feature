Feature: Admin enrichment provenance reporting endpoint
  Scenario: Authenticated admin can list enrichment provenance records
    Given a running server with session middleware
    And the client has an authenticated session
    And persisted enrichment provenance reporting records exist
    When the authenticated client requests enrichment provenance reporting
    Then the response is ok with an enrichment provenance payload

  Scenario: Admin enrichment reporting requires authentication
    Given a running server with session middleware
    When the unauthenticated client requests enrichment provenance reporting
    Then the response is unauthorized

  Scenario: Admin enrichment reporting rejects invalid limit values
    Given a running server with session middleware
    And the client has an authenticated session
    When the authenticated client requests enrichment provenance reporting with invalid limit
    Then the response is bad request

  Scenario: Admin enrichment reporting rejects over-max limit values
    Given a running server with session middleware
    And the client has an authenticated session
    When the authenticated client requests enrichment provenance reporting with over-max limit
    Then the response is bad request

  Scenario: Admin enrichment reporting rejects invalid before cursors
    Given a running server with session middleware
    And the client has an authenticated session
    When the authenticated client requests enrichment provenance reporting with invalid cursor
    Then the response is bad request

  Scenario: Admin enrichment reporting returns an empty payload when no rows exist
    Given a running server with session middleware
    And the client has an authenticated session
    And no enrichment provenance reporting records exist
    When the authenticated client requests enrichment provenance reporting
    Then the response is ok with an empty enrichment provenance payload

  Scenario: Admin enrichment reporting supports before-cursor pagination
    Given a running server with session middleware
    And the client has an authenticated session
    And persisted enrichment provenance reporting records exist
    When the authenticated client requests enrichment provenance reporting with limit and cursor
    Then the response includes a nextBefore cursor
    And the enrichment provenance query receives the expected limit and cursor

  Scenario: Admin enrichment reporting surfaces backend failures
    Given a running server with session middleware
    And the client has an authenticated session
    And enrichment provenance reporting is unavailable
    When the authenticated client requests enrichment provenance reporting
    Then the response is service unavailable
