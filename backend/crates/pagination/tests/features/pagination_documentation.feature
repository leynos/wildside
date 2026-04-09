Feature: Pagination documentation invariants
  The pagination crate documentation promises specific behaviour for limits,
  cursors, and error handling. These scenarios verify that the documented
  invariants hold at runtime.

  Scenario: Default limit is applied when no limit is provided
    Given pagination parameters without a limit
    Then the normalized limit equals DEFAULT_LIMIT

  Scenario: Maximum limit caps oversized requests
    Given pagination parameters with limit 500
    Then the normalized limit equals MAX_LIMIT

  Scenario: Zero limit is rejected with an error
    When pagination parameters are created with limit 0
    Then page parameter creation fails with InvalidLimit error

  Scenario: Invalid base64 token produces InvalidBase64 error
    Given an invalid base64 cursor token "not!valid"
    When the cursor is decoded
    Then decoding fails with InvalidBase64 error

  Scenario: Structurally invalid JSON produces Deserialize error
    Given a base64url token containing invalid JSON
    When the cursor is decoded
    Then decoding fails with Deserialize error

  Scenario: Error display strings are human-readable
    Given cursor decoding errors of different variants
    Then each error display string contains a descriptive message
