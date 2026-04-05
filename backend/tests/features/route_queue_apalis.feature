Feature: Apalis-backed RouteQueue adapter with PostgreSQL storage

  As a developer integrating the queue adapter
  I want the RouteQueue implementation to persist jobs to PostgreSQL
  So that enqueued work survives process restarts and can be consumed by workers

  Background:
    Given a test database with Apalis storage initialised

  Scenario: Successfully enqueue a plan
    When I enqueue a test plan
    Then the enqueue operation succeeds
    And the plan is persisted in the queue storage

  Scenario: Enqueue multiple distinct plans
    When I enqueue the first test plan
    And I enqueue the second test plan
    Then both enqueue operations succeed
    And both plans are persisted as separate jobs

  Scenario: Enqueue with invalid storage connection
    Given the queue adapter uses an invalid database connection
    When I attempt to enqueue a test plan
    Then the enqueue operation fails with an unavailable error

  Scenario: Enqueue the same plan twice creates independent jobs
    When I enqueue a test plan
    And I enqueue the same test plan again
    Then both enqueue operations succeed
    And two independent jobs exist in storage
