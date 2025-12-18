Feature: OpenAPI schema wrappers
  The backend uses schema wrapper types in the inbound HTTP layer to provide
  OpenAPI documentation without coupling domain types to the utoipa framework.

  Scenario: OpenAPI document registers schema wrappers
    Given the OpenAPI document is generated
    When the document is inspected
    Then the components section contains the Error schema wrapper
    And the components section contains the ErrorCode schema wrapper
    And the components section contains the User schema wrapper

  Scenario: Login endpoint uses schema wrapper types
    Given the OpenAPI document is generated
    When the document is inspected
    Then the login endpoint references ErrorSchema for error responses

  Scenario: List users endpoint uses schema wrapper types
    Given the OpenAPI document is generated
    When the document is inspected
    Then the list users endpoint references UserSchema for success response
    And the list users endpoint references ErrorSchema for error responses
