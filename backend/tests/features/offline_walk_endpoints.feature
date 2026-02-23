Feature: Offline bundle and walk-session endpoints
  Scenario: Authenticated client lists offline bundles for a device
    Given a running server with session middleware
    And the client has an authenticated session
    And the offline bundle query returns one bundle
    When the client lists offline bundles for the ios device
    Then the response is ok
    And the offline list response includes the configured bundle id
    And the offline list query captures session user and ios device

  Scenario: Offline bundle upsert returns a stable id and captures idempotency
    Given a running server with session middleware
    And the client has an authenticated session
    And the offline bundle command returns an upserted bundle
    When the client upserts an offline bundle with idempotency key
    Then the response is ok
    And the offline upsert response includes the configured bundle id
    And the offline upsert command captures the idempotency key

  Scenario: Offline bundle delete returns a stable id and captures idempotency
    Given a running server with session middleware
    And the client has an authenticated session
    And the offline bundle command returns a deleted bundle id
    When the client deletes an offline bundle with idempotency key
    Then the response is ok
    And the offline delete response includes the configured bundle id
    And the offline delete command captures the idempotency key

  Scenario: Offline bundle upsert surfaces replayed idempotency responses
    Given a running server with session middleware
    And the client has an authenticated session
    And the offline bundle command returns a replayed upsert bundle
    When the client upserts an offline bundle with idempotency key
    Then the response is ok
    And the response indicates replayed idempotent result

  Scenario: Offline bundle delete surfaces replayed idempotency responses
    Given a running server with session middleware
    And the client has an authenticated session
    And the offline bundle command returns a replayed deleted bundle id
    When the client deletes an offline bundle with idempotency key
    Then the response is ok
    And the response indicates replayed idempotent result

  Scenario: Walk session creation returns completion summary
    Given a running server with session middleware
    And the client has an authenticated session
    And the walk session command returns a completion summary
    When the client creates a walk session
    Then the response is ok
    And the walk session response includes the configured session id
    And the walk session response includes completion summary
    And the walk session command captures the session id

  Scenario: Offline list rejects missing device id
    Given a running server with session middleware
    And the client has an authenticated session
    When the client lists offline bundles without device id
    Then the response is bad request

  Scenario: Offline list rejects blank device id
    Given a running server with session middleware
    And the client has an authenticated session
    When the client lists offline bundles with blank device id
    Then the response is bad request with device id validation details

  Scenario: Offline upsert rejects malformed idempotency key
    Given a running server with session middleware
    And the client has an authenticated session
    When the client upserts an offline bundle with invalid idempotency key
    Then the response is bad request

  Scenario: Walk session creation requires authentication
    Given a running server with session middleware
    When the unauthenticated client creates a walk session
    Then the response is unauthorized
