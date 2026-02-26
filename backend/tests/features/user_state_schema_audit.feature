Feature: User state schema audit
  Roadmap 3.5.1 requires a schema audit that reports login, users, profile,
  and interests coverage plus migration decisions for unresolved gaps.

  Scenario: Baseline schema reports user-state migration gaps
    Given a migrated schema baseline
    When executing the user state schema audit
    Then login credentials storage is reported as missing
    And users and profile storage are reported as covered
    And interests migration decisions are required

  Scenario: Missing users table requires users and profile migrations
    Given a migrated schema baseline
    And the users table is missing
    When executing the user state schema audit
    Then users and profile migrations are required

  Scenario: Canonical interests model with revision needs no interests migrations
    Given a migrated schema baseline
    And interests use a canonical revisioned model
    When executing the user state schema audit
    Then interests migration decisions are not required
