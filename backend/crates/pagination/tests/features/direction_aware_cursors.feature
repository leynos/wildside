Feature: Direction-aware cursor pagination
  The pagination crate supports direction-aware cursors that indicate
  whether to fetch the next page (forward) or previous page (backward).

  Scenario: Next direction round-trips through encoding
    Given a composite ordering key
    And pagination direction Next
    When the key and direction are encoded into a cursor and decoded
    Then the decoded cursor has direction Next
    And the decoded cursor key matches the original key

  Scenario: Prev direction round-trips through encoding
    Given a composite ordering key
    And pagination direction Prev
    When the key and direction are encoded into a cursor and decoded
    Then the decoded cursor has direction Prev
    And the decoded cursor key matches the original key

  Scenario: Cursor without explicit direction defaults to Next
    Given a composite ordering key
    When the key is encoded into an opaque cursor and decoded again
    Then the decoded cursor has direction Next
