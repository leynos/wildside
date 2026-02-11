Feature: Catalogue and descriptor ingestion
  Scenario: Catalogue and descriptor ingestion supports success failure and nullable edge cases
    Given a Diesel-backed catalogue and descriptor ingestion repository
    When the repositories upsert validated catalogue and descriptor snapshots
    Then catalogue and descriptor rows are stored with localization and semantic icon keys
    When the tags table is dropped and a tag upsert is attempted
    Then the descriptor repository reports a query error
    When a community pick without route and user references is upserted
    Then the stored community pick keeps null route and user references
