Feature: Session key fingerprinting
  The backend computes a truncated SHA-256 fingerprint of the session signing
  key for operational visibility. Fingerprints enable operators to verify which
  key is active across replicas.

  Scenario: Fingerprint is deterministic for the same key
    Given a session key derived from fixed bytes
    When the fingerprint is computed twice
    Then both fingerprints are identical

  Scenario: Different keys produce different fingerprints
    Given a session key derived from bytes 'a'
    And another session key derived from bytes 'b'
    When fingerprints are computed for both keys
    Then the fingerprints differ

  Scenario: Fingerprint has correct format
    Given a randomly generated session key
    When the fingerprint is computed
    Then the fingerprint is 16 characters long
    And the fingerprint contains only hexadecimal characters
    And the fingerprint is lowercase

  Scenario: Session settings include fingerprint
    Given a release build configuration
    And SESSION_COOKIE_SECURE is set to 1
    And SESSION_SAMESITE is set to Strict
    And SESSION_ALLOW_EPHEMERAL is set to 0
    And a session key file with 64 bytes
    When the session configuration is loaded
    Then the configuration load succeeds
    And the settings include a non-empty fingerprint
