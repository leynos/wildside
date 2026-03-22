Feature: Pagination crate foundation
  The pagination crate provides opaque cursor tokens, normalized page
  parameters, and paginated envelopes with navigation links.

  Scenario: Cursor round-trips for a composite ordering key
    Given a composite ordering key
    When the key is encoded into an opaque cursor and decoded again
    Then the decoded cursor key matches the original key

  Scenario: Malformed cursor tokens are rejected
    Given a malformed opaque cursor token
    When the cursor is decoded
    Then cursor decoding fails

  Scenario: Page parameters use the shared default and maximum limits
    Given pagination parameters without a limit
    When the parameters are normalized
    Then the normalized limit is 20
    When pagination parameters request limit 500
    And the parameters are normalized
    Then the normalized limit is 100

  Scenario: Paginated envelopes build self next and prev links
    Given normalized pagination parameters with cursor "current-token"
    And a request URL with filter query parameters
    When a paginated envelope is built with next and prev cursors
    Then the self link preserves the current cursor and filter
    And the next link uses the next cursor
    And the prev link uses the prev cursor
