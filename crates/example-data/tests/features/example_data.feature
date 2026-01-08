Feature: Example data generation
  The example-data crate provides deterministic user generation from seed
  registries for demonstration purposes.

  Scenario: Valid registry parses successfully
    Given a valid seed registry JSON
    When the registry is parsed
    Then parsing succeeds
    And the registry contains the expected seed definitions

  Scenario: Deterministic generation produces identical users
    Given a valid seed registry
    And a seed definition with seed 42
    When users are generated twice
    Then both generations produce identical users

  Scenario: Generated display names are valid
    Given a valid seed registry
    And a seed definition
    When users are generated
    Then all display names satisfy backend constraints

  Scenario: Interest theme selection stays within registry
    Given a valid seed registry with interest theme IDs
    And a seed definition
    When users are generated
    Then all interest theme IDs exist in the registry

  Scenario: Invalid JSON fails parsing
    Given malformed JSON
    When the registry is parsed
    Then parsing fails with a parse error

  Scenario: Empty seeds array fails parsing
    Given registry JSON with empty seeds array
    When the registry is parsed
    Then parsing fails with empty seeds error

  Scenario: Invalid UUID in interest themes fails parsing
    Given registry JSON with invalid interest theme UUID
    When the registry is parsed
    Then parsing fails with invalid UUID error
