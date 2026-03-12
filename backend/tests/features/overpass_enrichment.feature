Feature: Overpass enrichment worker
  Scenario: Overpass enrichment persists fetched POIs
    Given a Diesel-backed Overpass enrichment worker with successful source data
    When an enrichment job runs for launch-a bounds
    Then the worker reports a successful enrichment outcome
    And enrichment POIs are persisted
    And an enrichment success metric is recorded

  Scenario: Overpass enrichment respects request quota limits
    Given a Diesel-backed Overpass enrichment worker with exhausted request quota
    When an enrichment job runs for launch-a bounds
    Then the worker fails with service unavailable
    And no Overpass source calls were made
    And an enrichment quota failure metric is recorded

  Scenario: Overpass enrichment opens the circuit after repeated failures
    Given a Diesel-backed Overpass enrichment worker with failing source responses
    When two enrichment jobs run for launch-a bounds
    And a third enrichment job runs for launch-a bounds
    Then the third job fails fast with service unavailable
    And the source call count is two
    And an enrichment circuit-open metric is recorded

  Scenario: Overpass enrichment recovers after circuit cooldown
    Given a Diesel-backed Overpass enrichment worker with recovery source responses
    When one enrichment job fails for launch-a bounds
    And the worker clock advances by 61 seconds
    And an enrichment job runs for launch-a bounds
    Then the worker reports a successful enrichment outcome
    And the source call count is two

  Scenario: Overpass enrichment reports retry exhaustion after transient failures
    Given a Diesel-backed Overpass enrichment worker with retry-exhaustion source responses
    When an enrichment job runs for launch-a bounds
    Then the worker fails with service unavailable
    And the source call count is two
    And an enrichment retry-exhausted metric is recorded

  Scenario: Overpass enrichment semaphore limits concurrent source calls
    Given a Diesel-backed Overpass enrichment worker with semaphore-blocking source responses
    When two enrichment jobs run concurrently for launch-a bounds
    Then both concurrent jobs complete successfully
    And the max observed concurrent source calls is one

  Scenario: Overpass enrichment persists provenance metadata
    Given a Diesel-backed Overpass enrichment worker with successful source data and provenance capture
    When an enrichment job runs for launch-a bounds
    Then enrichment provenance is persisted with source URL timestamp and bounding box

  Scenario: Overpass enrichment reports provenance persistence failures
    Given a Diesel-backed Overpass enrichment worker with unavailable provenance persistence
    When an enrichment job runs for launch-a bounds
    Then enrichment provenance write failures surface internal errors

  Scenario: Overpass enrichment persists provenance even with zero POIs
    Given a Diesel-backed Overpass enrichment worker with successful zero-POI source data and provenance capture
    When an enrichment job runs for launch-a bounds
    Then enrichment provenance entries are written even when zero POIs are returned
