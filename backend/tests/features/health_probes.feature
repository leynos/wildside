Feature: Health probes
  Orchestrators observe Wildside through stable liveness and readiness probes.

  Scenario: Readiness rejects traffic before startup completes
    Given a live Wildside runtime
    When the readiness probe is requested
    Then the probe response status is 503
    And the probe response is not cacheable

  Scenario: Readiness accepts traffic after startup completes
    Given a live Wildside runtime
    And the runtime is ready
    When the readiness probe is requested
    Then the probe response status is 200
    And the probe response is not cacheable

  Scenario: Liveness accepts traffic while the runtime is live
    Given a live Wildside runtime
    When the liveness probe is requested
    Then the probe response status is 200
    And the probe response is not cacheable

  Scenario: Liveness fails after the runtime is marked unhealthy
    Given a live Wildside runtime
    And the runtime is unhealthy
    When the liveness probe is requested
    Then the probe response status is 503
    And the probe response is not cacheable
