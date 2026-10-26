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
    When the operator lists dead jobs
    Then Apalis does not fetch the row
    And the dead listing contains it as Unclassified

  Scenario: A requeued fatal job regains its full retry budget
    Given a job has been dead-lettered by a fatal error
    And the dead job has an existing requeue count and operator metadata
    And the job now fails with a retryable error
    When the operator requeues the dead job
    Then the requeue restores its budget and preserves unrelated metadata
    When the worker processes the queue
    Then the job is dead because its retries were exhausted
    And the requeue count remains one
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
    Then the dead listing contains it as Unclassified

  Scenario: Dead-job listing is newest first and respects its limit
    Given two dead jobs exist
    When the operator lists dead jobs with a limit of 1
    Then only the newest dead job is listed

  Scenario: Requeue rejects a dead job from an unknown queue
    Given a dead job belongs to an unconfigured queue
    When the operator requeues that dead job
    Then the operation fails because its queue has no retry budget
