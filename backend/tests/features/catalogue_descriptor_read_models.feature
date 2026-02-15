Feature: Catalogue and descriptor read model repositories
  Scenario: Read repositories return seeded snapshots and handle empty and malformed data
    Given a Diesel-backed catalogue and descriptor read repository
    When catalogue and descriptor data is seeded via ingestion
    And the catalogue snapshot is read
    Then the explore snapshot contains expected categories themes and routes
    And the community pick is present with correct localization
    When the descriptor snapshot is read
    Then the descriptor snapshot contains expected tags badges and presets
    When all catalogue tables are truncated
    And the catalogue snapshot is read
    Then the explore snapshot returns empty collections
    When a malformed localization row is inserted directly
    And the catalogue snapshot is read
    Then the catalogue read repository reports a query error
    When a malformed descriptor localization row is inserted directly
    And the descriptor snapshot is read
    Then the descriptor read repository reports a query error
