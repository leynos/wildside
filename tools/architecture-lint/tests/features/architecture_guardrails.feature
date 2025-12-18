Feature: Architecture guardrails
  The backend is a hexagonal modular monolith. To keep the boundaries visible
  during feature delivery we run a repo-local lint that rejects adapter and
  infrastructure dependencies in the wrong layer.

  Scenario: Inbound adapters cannot depend on outbound adapters
    Given an inbound module that imports the outbound layer
    When the architecture lint runs
    Then the lint fails due to outbound access from inbound

  Scenario: Inbound adapters cannot depend on infrastructure crates directly
    Given an inbound module that imports Diesel directly
    When the architecture lint runs
    Then the lint fails due to infrastructure crate usage

  Scenario: Domain code cannot depend on framework crates
    Given a domain module that imports Actix Web
    When the architecture lint runs
    Then the lint fails due to framework crate usage in the domain

  Scenario: Outbound adapters cannot depend on inbound adapters
    Given an outbound module that imports the inbound layer
    When the architecture lint runs
    Then the lint fails due to inbound access from outbound

  Scenario: Well-formed module dependencies pass
    Given valid domain, inbound, and outbound modules
    When the architecture lint runs
    Then the lint succeeds

  Scenario: Mixed valid and invalid modules produce multiple violations
    Given valid modules mixed with multiple boundary violations
    When the architecture lint runs
    Then the lint fails
    And all boundary violations are reported

  Scenario: Domain code cannot depend on utoipa
    Given a domain module that imports utoipa
    When the architecture lint runs
    Then the lint fails due to utoipa usage in the domain
