Feature: Persisted domain job envelopes

  Scenario: Enqueue and decode typed jobs at the PostgreSQL boundary
    Given valid generate-route and enrichment jobs
    When I enqueue both jobs through Apalis PostgreSQL
    Then both JSON payloads are persisted in apalis.jobs
    And decode_job restores both typed job envelopes
