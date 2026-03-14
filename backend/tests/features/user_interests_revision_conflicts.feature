Feature: Revision-safe interests updates
  Interests writes share the user preferences aggregate revision and must
  expose optimistic concurrency conflicts to callers.

  Scenario: First interests write creates revision 1
    Given db-present startup mode backed by embedded postgres
    When the client writes interests for the first time
    Then the first interests response includes revision 1

  Scenario: Matching expected revision advances interests revision
    Given db-present startup mode backed by embedded postgres
    When the client writes interests twice using the returned revision
    Then the second interests response includes revision 2

  Scenario: Stale expected revision returns a conflict
    Given db-present startup mode backed by embedded postgres
    And existing preferences revision 2
    When the client writes interests with stale expected revision 1
    Then the response is a conflict with expected revision 1 and actual revision 2

  Scenario: Missing expected revision after preferences exist returns a conflict
    Given db-present startup mode backed by embedded postgres
    And existing preferences revision 1 with preserved safety and unit settings
    When the client writes interests without expected revision after preferences exist
    Then the response is a conflict with missing expected revision and actual revision 1

  Scenario: Interests updates preserve non-interest preferences fields
    Given db-present startup mode backed by embedded postgres
    And existing preferences revision 1 with preserved safety and unit settings
    When the client updates interests and then fetches preferences
    Then the fetched preferences preserve safety and unit settings while advancing revision 2
