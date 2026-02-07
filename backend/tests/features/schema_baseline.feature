Feature: Schema baseline migration
  Data platform baseline migrations must create the catalogue and descriptor
  schema with required constraints and spatial indexes.

  Scenario: Baseline tables are materialized
    Given a migrated schema baseline
    When listing baseline tables
    Then all required baseline tables are present

  Scenario: Spatial and JSON indexes are present
    Given a migrated schema baseline
    When querying baseline indexes
    Then GiST and GIN indexes are present

  Scenario: Duplicate route positions are rejected
    Given a migrated schema baseline
    And a seeded route with two points of interest
    When inserting duplicate route positions
    Then insertion fails with a unique constraint violation

  Scenario: Duplicate POI composite keys are rejected
    Given a migrated schema baseline
    And an existing point of interest
    When inserting a duplicate point of interest
    Then insertion fails with a unique constraint violation
