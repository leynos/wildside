Feature: Offline bundle and walk session repositories
  Scenario: Repositories persist manifests and completion summaries with query-error mapping
    Given postgres-backed offline bundle and walk session repositories
    When a route bundle and an anonymous region bundle are saved
    And bundles are listed for the owner and device
    Then the owner listing includes the route bundle only
    When anonymous bundles are listed for the region device
    Then the anonymous listing includes the region bundle only
    When a completed walk session is saved and queried
    Then the walk session and completion summary are returned
    When the offline bundle table is dropped and an offline save is attempted
    Then the offline repository reports a query error
    When the walk session table is dropped and a walk save is attempted
    Then the walk session repository reports a query error
