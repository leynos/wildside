Feature: Domain job structs for queue payloads

  Scenario: Build a generate-route job from a submission and enqueue via stub
    Given a valid route submission
    When I build and enqueue a generate-route job through the stub queue
    Then the stub enqueue succeeds

  Scenario: Build an enrichment job and observe its queue payload
    Given a valid enrichment job
    When I enqueue the enrichment job through the fake Apalis queue
    Then the fake queue records the enrichment JSON payload

  Scenario: Surface a serialization rejection
    Given a plan that fails serialization
    When I enqueue the failing plan through the fake Apalis queue
    Then the queue returns a rejected dispatch error

  Scenario: Convert an enrichment job to an enrichment request
    Given a valid enrichment job
    When I convert the enrichment job to an enrichment request
    Then the enrichment request preserves the job fields
