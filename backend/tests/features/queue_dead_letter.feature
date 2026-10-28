Feature: Dead-letter listing, requeue, and discard

  Scenario: Dead jobs are listed with their failure context
    Given a job has been dead-lettered after exhausting its retries
    When the operator lists dead jobs
    Then the listing contains the job with its reason, attempt count and last error

  Scenario: A requeued dead job runs again and can succeed
    Given a job has been dead-lettered after exhausting its retries
    And the underlying fault has been fixed
    When the operator requeues the dead job
    And the worker processes the queue
    Then the job completes with status "Done"
    And the job retains one requeue in its metadata

  Scenario: A legacy failed row with a null attempt limit is listed
    Given a disposable legacy schema permits a null max_attempts value
    And a failed row has 3 attempts, below Apalis's null fallback of 25
    And a pending control row exists
    When the worker polls the queue for a bounded period
    Then the pending control row is processed
    And Apalis does not fetch the legacy row
    When the operator lists dead jobs
    Then the dead listing contains the legacy row as Unclassified

  Scenario: A requeued fatal job regains its full retry budget
    Given a job has been dead-lettered by a fatal error
    And the dead job has an existing requeue count and operator metadata
    And the job now fails with a retryable error
    When the operator requeues the dead job
    Then the requeue restores its budget and preserves unrelated metadata
    When the worker processes the queue
    Then the job is dead because its retries were exhausted
    And the requeue counter advances to five
    And the fatal reason has been replaced with exhausted
    And the handler runs the queue's full number of attempts

  Scenario: Discard rejects a live job without changing it
    Given a pending job exists
    When the operator discards that job
    Then the operation fails because the job is not dead
    And the job's state is unchanged

  Scenario: Requeuing a live job is rejected
    Given a pending job exists
    When the operator requeues that job
    Then the operation fails because the job is not dead
    And the job's state is unchanged

  Scenario: A discarded dead job is gone
    Given a job has been dead-lettered after exhausting its retries
    When the operator discards the dead job
    Then the listing is empty
    And the dead job row is gone

  Scenario: Listing dead jobs ignores live jobs
    Given a pending job and a running job exist
    When the operator lists dead jobs
    Then the listing is empty

  Scenario: A legacy or malformed terminal reason is unclassified
    Given a failed dead row has no recognized terminal reason
    When the operator lists dead jobs
    Then the dead listing contains the legacy row as Unclassified

  Scenario: A malformed requeue count does not hide valid dead jobs
    Given malformed requeue counts and another dead job exist
    When the operator lists dead jobs with a limit of 1
    Then the listing contains only the valid dead job

  Scenario: A negative attempt count does not fail the dead-job listing
    Given a dead row has a negative attempt count
    When the operator lists dead jobs
    Then the listing is empty

  Scenario: Requeue rejects malformed counter metadata without changing the row
    Given a dead job has a malformed requeue count
    When the operator requeues the invalid dead job
    Then the operation fails because the dead job data is invalid
    And the job's state is unchanged

  Scenario: Requeue rejects negative attempts without changing the row
    Given a dead row has a negative attempt count
    When the operator requeues the invalid dead job
    Then the operation fails because the dead job data is invalid
    And the job's state is unchanged

  Scenario: Dead-job listing is newest first and respects its limit
    Given two dead jobs exist
    When the operator lists dead jobs with a limit of 1
    Then only the newest dead job is listed

  Scenario: Requeue rejects a dead job from an unknown queue
    Given a dead job belongs to an unconfigured queue
    When the operator requeues the dead job
    Then the operation fails because its queue has no retry budget
