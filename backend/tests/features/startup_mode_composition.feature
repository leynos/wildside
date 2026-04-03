Feature: Startup mode composition matrix for all HTTP-facing ports
  Roadmap item 3.5.5 requires comprehensive startup-mode composition coverage
  for all 16 ports in `HttpStatePorts` and `HttpStateExtraPorts`, proving that
  adapter selection remains deterministic for both fixture-fallback and
  DB-present startup modes. This feature exercises representative endpoints
  for each port group at the HTTP boundary with embedded PostgreSQL backing.

  Scenario: Fixture-fallback startup preserves fixture contracts for all port groups
    Given fixture-fallback startup mode without a database pool
    When executing requests against all major endpoint groups
    Then all responses match fixture fallback contracts

  Scenario: DB-present startup preserves DB-backed contracts for all port groups
    Given db-present startup mode backed by embedded postgres
    When executing requests against all major endpoint groups
    Then all responses match DB-backed contracts

  Scenario: DB-present startup produces stable error envelopes when critical schemas are missing
    Given db-present startup mode backed by embedded postgres
    And the users table is missing in db-present mode
    When executing requests against user-dependent endpoints
    Then responses produce stable error envelopes rather than fixture data

  Scenario: Validation error envelopes remain stable across both startup modes
    Given fixture-fallback startup mode without a database pool
    When executing requests with invalid input against endpoints
    Then validation error envelopes are identical to db-present validation errors
    Given db-present startup mode backed by embedded postgres
    When executing requests with invalid input against endpoints
    Then validation error envelopes remain stable
