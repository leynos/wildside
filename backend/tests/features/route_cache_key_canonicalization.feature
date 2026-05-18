Feature: Route cache key canonicalization
  The backend derives Redis cache keys from route request payloads in a stable
  domain-owned way so semantically equivalent requests share cached plans.

  Scenario: Semantically equivalent route requests share one Redis cache slot
    Given a running Redis-backed route cache
    When semantically equivalent route requests are canonicalized into cache keys
    And a plan is stored under the first canonical cache key
    And the cache is read with the second canonical cache key
    Then both route requests share the same canonical cache key
    And the canonical cache key uses the route v1 sha256 format
    And the same plan is returned from the cache
