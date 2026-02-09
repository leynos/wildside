Feature: ER diagram snapshots from migrations
  Schema traceability requires generated ER snapshots based on the current
  migration-backed PostgreSQL schema.

  Scenario: Snapshots are generated from migrated schema
    Given a migration-backed temporary database
    And an empty ER snapshot output directory
    When ER snapshots are generated
    Then Mermaid and SVG snapshot files are created

  Scenario: Snapshot generation reports renderer failures
    Given a migration-backed temporary database
    And an empty ER snapshot output directory
    When ER snapshots are generated with a missing renderer command
    Then generation fails with a renderer error
    And no snapshot files are written

  Scenario: Snapshot generation is deterministic across reruns
    Given a migration-backed temporary database
    And an empty ER snapshot output directory
    When ER snapshots are generated twice
    Then the Mermaid snapshot content is identical across runs
