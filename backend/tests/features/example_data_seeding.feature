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
