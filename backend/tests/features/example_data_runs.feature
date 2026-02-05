Feature: Example data seeding guard
  The backend tracks which example data seeds have been applied to prevent
  duplicate seeding on concurrent startups or restarts. The repository uses
  idempotent semantics to ensure each seed is applied at most once.

  Scenario: First seed attempt succeeds
    Given a fresh database for example data runs
    When a seed is recorded for "mossy-owl"
    Then the result is "applied"

  Scenario: Duplicate seed attempt is detected
    Given a database with seed "mossy-owl" already recorded
    When a seed is recorded for "mossy-owl"
    Then the result is "already seeded"

  Scenario: Different seeds are independent
    Given a database with seed "mossy-owl" already recorded
    When a seed is recorded for "clever-fox"
    Then the result is "applied"

  Scenario: Query returns false for unknown seeds
    Given a fresh database for example data runs
    When checking if seed "unknown-seed" exists
    Then the existence check returns false

  Scenario: Query returns true for recorded seeds
    Given a database with seed "mossy-owl" already recorded
    When checking if seed "mossy-owl" exists
    Then the existence check returns true
