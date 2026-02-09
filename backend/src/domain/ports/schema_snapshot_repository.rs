//! Port abstraction for loading schema metadata for ER snapshots.

use crate::domain::er_diagram::SchemaDiagram;

use super::define_port_error;

define_port_error! {
    /// Errors raised when loading schema metadata for diagram snapshots.
    pub enum SchemaSnapshotRepositoryError {
        /// Connection to the backing datastore failed.
        Connection { message: String } =>
            "schema snapshot connection failed: {message}",
        /// Schema introspection query failed.
        Query { message: String } =>
            "schema snapshot query failed: {message}",
    }
}

/// Port for reading schema metadata from the active persistence backend.
#[cfg_attr(test, mockall::automock)]
pub trait SchemaSnapshotRepository: Send + Sync {
    /// Load an ER-focused schema snapshot from the backing store.
    fn load_schema_diagram(&self) -> Result<SchemaDiagram, SchemaSnapshotRepositoryError>;
}

/// Fixture implementation returning an empty diagram.
#[derive(Debug, Clone, Default)]
pub struct FixtureSchemaSnapshotRepository;

impl SchemaSnapshotRepository for FixtureSchemaSnapshotRepository {
    fn load_schema_diagram(&self) -> Result<SchemaDiagram, SchemaSnapshotRepositoryError> {
        Ok(SchemaDiagram {
            tables: Vec::new(),
            relationships: Vec::new(),
        })
    }
}
