Feature: Queue retry policy with durable exponential backoff

  Scenario: A transiently failing job is retried before it succeeds
    Given a queue with a retry policy of 4 attempts and millisecond backoff
    And a job whose handler fails twice then succeeds
    When the worker processes the queue
    Then the handler runs three times with attempts 1, 2 and 3
    And each rerun starts no earlier than its persisted retry time
    And the job completes with status "Done"
    And the acknowledgement preserves the final attempt state
    And retry metrics contain 2 scheduled, 0 exhausted and 0 fatal outcomes

  Scenario: A retryable failure is dead-lettered after exhausting attempts
    Given a queue with a retry policy of 3 attempts and millisecond backoff
    And a job whose handler always fails with a retryable error
    When the worker processes the queue
    Then the handler runs exactly 3 times
    And the row is stored as Killed and satisfies the dead predicate
    And the persisted terminal reason is "exhausted"
    And the acknowledgement preserves the final attempt state
    And further polling delivers no more runs
    And retry metrics contain 2 scheduled, 1 exhausted and 0 fatal outcomes

  Scenario: A fatal failure is dead-lettered immediately
    Given a queue with a retry policy of 3 attempts and millisecond backoff
    And a job whose handler fails with a fatal error
    And the job has an unrelated metadata marker
    When the worker processes the queue
    Then the handler runs exactly once
    And the row is stored as Failed and satisfies the dead predicate
    And the persisted terminal reason is fatal
    And the acknowledgement preserves the final attempt state
    And unrelated metadata survives acknowledgement
    And further polling delivers no more runs
    And retry metrics contain 0 scheduled, 0 exhausted and 1 fatal outcomes

  Scenario: A fatal failure at the attempt limit stores Killed
    Given a queue with a retry policy of one attempt and millisecond backoff
    And a job whose handler fails with a fatal error
    When the worker processes the queue
    Then the handler runs exactly once
    And the row is stored as Killed and satisfies the dead predicate
    And the persisted terminal reason is fatal
    And the acknowledgement preserves the final attempt state
    And further polling delivers no more runs
    And retry metrics contain 0 scheduled, 0 exhausted and 1 fatal outcomes

  Scenario: A lower policy limit dead-letters a pre-existing job
    Given a queue with a retry policy of 3 attempts and millisecond backoff
    And a pre-existing job permits 25 attempts
    And its handler always fails with a retryable error
    When the worker processes the queue
    Then the handler runs exactly 3 times
    And the row is stored as Failed and satisfies the dead predicate
    And the persisted terminal reason is "exhausted"
    And the acknowledgement preserves the final attempt state
    And further polling delivers no more runs
    And retry metrics contain 2 scheduled, 1 exhausted and 0 fatal outcomes

  Scenario: An enqueued job carries the queue retry budget
    Given a queue with a retry policy of 5 attempts and millisecond backoff
    When a job is enqueued through the route queue adapter
    Then the stored job allows at most 5 attempts
