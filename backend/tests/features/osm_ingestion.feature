Feature: OSM ingestion command
  Scenario: OSM ingestion persists geofenced POIs and provenance
    Given a Diesel-backed OSM ingestion command service
    When an ingest run executes for geofence launch-a
    Then the command reports an executed ingest outcome
    And geofenced POIs and provenance are persisted

  Scenario: OSM ingestion reruns deterministically for the same geofence and digest
    Given a Diesel-backed OSM ingestion command service
    When an ingest run executes for geofence launch-a
    Then the command reports an executed ingest outcome
    And geofenced POIs and provenance are persisted
    When the same ingest reruns for geofence launch-a and digest
    Then the command reports a replayed ingest outcome
    And persisted row counts stay unchanged

  Scenario: OSM ingestion fails when provenance persistence is unavailable
    Given a Diesel-backed OSM ingestion command service
    When the provenance table is dropped
    And an ingest run executes for geofence launch-b
    Then the command fails with service unavailable
