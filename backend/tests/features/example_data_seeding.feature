Feature: Example data startup seeding
  The backend should seed example users and preferences exactly once per
  seed name so demo data is available without duplicating records.

  Scenario: First seed run applies example data
    Given a fresh database
    And a seed registry with seed "mossy-owl"
    When startup seeding runs for "mossy-owl"
    Then the seeding result is "applied"
    And 2 users are stored
    And 2 preferences are stored

  Scenario: Seed run is skipped when already applied
    Given a fresh database
    And a seed registry with seed "mossy-owl"
    When startup seeding runs for "mossy-owl"
    And startup seeding runs again for "mossy-owl"
    Then the seeding result is "already seeded"

  Scenario: Missing seed returns an error
    Given a fresh database
    And a seed registry with seed "mossy-owl"
    When startup seeding runs for "missing-seed"
    Then a seeding error is returned

  Scenario: Seeding is skipped when disabled
    Given a fresh database
    And a seed registry with seed "mossy-owl"
    And example data seeding is disabled
    When startup seeding runs for "mossy-owl"
    Then startup seeding is skipped
    And 0 users are stored
    And 0 preferences are stored

  Scenario: Seeding is skipped when database is missing
    Given a fresh database
    And a seed registry with seed "mossy-owl"
    And the database is unavailable
    When startup seeding runs for "mossy-owl"
    Then startup seeding is skipped
    And 0 users are stored
    And 0 preferences are stored

  Scenario: Invalid registry path returns an error
    Given a fresh database
    And an invalid registry path
    When startup seeding runs for "mossy-owl"
    Then a seeding error is returned
