Feature: Users list pagination
  Scenario: First users page exposes the next link only
    Given db-present startup mode with five ordered users
    When the client requests the first users page with limit 2
    Then the users response is ok
    And the users page contains users 1 through 2
    And the users page includes a next link and omits the prev link

  Scenario: Following next reaches the final users page
    Given db-present startup mode with five ordered users
    When the client follows users next links with limit 2 until the final page
    Then the users response is ok
    And the users page contains user 5 only
    And the users page includes a prev link and omits the next link
    And forward traversal returned every seeded user once

  Scenario: Following prev from the final users page returns the prior page
    Given db-present startup mode with five ordered users
    When the client follows next then prev users links with limit 2
    Then the users response is ok
    And the users page contains users 3 through 4

  Scenario: Oversized users page limit is rejected
    Given db-present startup mode with five ordered users
    When the client requests the users list with limit 200
    Then the users response is bad request with invalid_limit details

  Scenario: Invalid users cursor is rejected
    Given db-present startup mode with five ordered users
    When the client requests the users list with an invalid cursor
    Then the users response is bad request with invalid_cursor details

  Scenario: Users list requires a session
    Given db-present startup mode with five ordered users
    When the client requests the users list without a session
    Then the users response is unauthorised
