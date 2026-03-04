Feature: User state startup modes for profile and interests endpoints
  Roadmap item 3.5.3 requires startup to preserve stable contracts when
  DB-backed profile/interests wiring is active.

  Scenario: DB-present startup remains stable when interests schema is missing
    Given db-present startup mode backed by embedded postgres
    And the interests schema is missing in db-present mode
    When executing a valid login, profile, and interests request
    Then the responses preserve a stable startup error or fallback contract

  Scenario: DB-present startup keeps interestThemeIds validation envelope stable
    Given db-present startup mode backed by embedded postgres
    When executing a login, profile, and interests request with too many interestThemeIds
    Then the interests validation error envelope remains stable
