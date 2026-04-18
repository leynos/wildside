Feature: Redis-backed route cache
  The backend uses a Redis-backed adapter to satisfy the `RouteCache` port
  while keeping Redis details outside the domain layer.

  Scenario: Stored plans round-trip through Redis
    Given a running Redis-backed route cache
    When a plan is stored under cache key "route:happy"
    And the cache is read for key "route:happy"
    Then the same plan is returned from the cache

  Scenario: Missing keys return a cache miss
    Given a running Redis-backed route cache
    When the cache is read for key "route:missing"
    Then the cache reports a miss

  Scenario: Malformed cached bytes surface as serialization failures
    Given a Redis-backed route cache with malformed cached bytes
    When the cache is read for key "route:corrupt"
    Then the error maps to a serialization failure

  Scenario: Unreachable Redis surfaces as a backend failure
    Given an unavailable Redis-backed route cache
    When the unavailable cache is read for key "route:down"
    Then the error maps to a backend failure

  Scenario: Distinct cache keys do not overwrite each other
    Given a running Redis-backed route cache
    When distinct plans are stored under cache keys "route:first" and "route:second"
    And both cache keys are read back
    Then each cache key keeps its own plan

  Scenario: Jittered writes produce varying TTLs
    Given a running Redis-backed route cache
    When five plans are stored under distinct cache keys
    Then not all recorded TTLs are identical
    And all recorded TTLs fall within the configured jitter window
