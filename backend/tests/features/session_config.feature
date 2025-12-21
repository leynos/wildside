Feature: Session configuration toggles
  Release builds must enforce explicit session toggles and reject insecure or
  missing configuration.

  Scenario: Release build requires SESSION_COOKIE_SECURE
    Given a release build configuration
    And SESSION_SAMESITE is set to Strict
    And SESSION_ALLOW_EPHEMERAL is set to 0
    And a session key file with 64 bytes
    When the session configuration is loaded
    Then the configuration load fails due to missing SESSION_COOKIE_SECURE

  Scenario: Release build rejects ephemeral session keys
    Given a release build configuration
    And SESSION_COOKIE_SECURE is set to 1
    And SESSION_SAMESITE is set to Strict
    And SESSION_ALLOW_EPHEMERAL is set to 1
    And a session key file with 64 bytes
    When the session configuration is loaded
    Then the configuration load fails because ephemeral keys are not allowed

  Scenario: Release build requires secure cookies for SameSite None
    Given a release build configuration
    And SESSION_COOKIE_SECURE is set to 0
    And SESSION_SAMESITE is set to None
    And SESSION_ALLOW_EPHEMERAL is set to 0
    And a session key file with 64 bytes
    When the session configuration is loaded
    Then the configuration load fails because SameSite=None requires secure cookies

  Scenario: Release build rejects short session keys
    Given a release build configuration
    And SESSION_COOKIE_SECURE is set to 1
    And SESSION_SAMESITE is set to Strict
    And SESSION_ALLOW_EPHEMERAL is set to 0
    And a session key file with 32 bytes
    When the session configuration is loaded
    Then the configuration load fails because the key is too short

  Scenario: Release build accepts valid session configuration
    Given a release build configuration
    And SESSION_COOKIE_SECURE is set to 1
    And SESSION_SAMESITE is set to Strict
    And SESSION_ALLOW_EPHEMERAL is set to 0
    And a session key file with 64 bytes
    When the session configuration is loaded
    Then the configuration load succeeds
    And the cookie secure flag is true
    And the SameSite policy is Strict
