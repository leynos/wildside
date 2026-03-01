Feature: User state startup modes for login and users endpoints
  Roadmap item 3.5.2 requires startup to preserve fixture fallback behaviour
  while enabling DB-present mode for login and users contracts.

  Scenario: Fixture fallback startup keeps fixture login and users behaviour
    Given fixture-fallback startup mode
    When executing a valid login and users request
    Then login succeeds with a session cookie
    And the users response matches fixture fallback payload

  Scenario: DB-present startup rejects invalid credentials
    Given db-present startup mode backed by embedded postgres
    When executing an invalid login request
    Then the login response is unauthorized with stable error envelope

  Scenario: DB-present startup remains stable when users schema is missing
    Given db-present startup mode backed by embedded postgres
    And the users table is missing in db-present mode
    When executing a valid login and users request
    Then the responses preserve a stable startup error or fallback contract
