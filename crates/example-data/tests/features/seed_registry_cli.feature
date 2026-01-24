Feature: Seed registry CLI
  The seed registry CLI appends named seeds to the JSON registry so demo data
  can be generated without manual edits.

  Scenario: Add a seed with a generated name
    Given a seed registry file
    When the seed registry CLI adds a seed using RNG value 2026
    Then the registry contains the generated seed name
    And the CLI reports success

  Scenario: Add a seed with an explicit name
    Given a seed registry file
    When the seed registry CLI adds a seed named "driftwood-harbour"
    Then the registry contains seed named "driftwood-harbour"
    And the CLI reports success

  Scenario: Duplicate seed names are rejected
    Given a seed registry file with seed named "mossy-owl"
    When the seed registry CLI adds a seed named "mossy-owl"
    Then the CLI reports a duplicate seed error
    And the registry remains unchanged

  Scenario: Invalid registry JSON fails
    Given an invalid seed registry file
    When the seed registry CLI adds a seed named "river-stone"
    Then the CLI reports a registry parse error
