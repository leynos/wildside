//! PostgreSQL-backed schema introspection adapter for ER snapshot generation.

use std::collections::BTreeMap;

use postgres::{Client, NoTls};

use crate::domain::er_diagram::{SchemaColumn, SchemaDiagram, SchemaRelationship, SchemaTable};
use crate::domain::ports::{SchemaSnapshotRepository, SchemaSnapshotRepositoryError};

/// Reads schema metadata from PostgreSQL system catalogs.
#[derive(Debug, Clone)]
pub struct PostgresSchemaSnapshotRepository {
    database_url: String,
}

impl PostgresSchemaSnapshotRepository {
    /// Construct a repository from a PostgreSQL connection URL.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use backend::outbound::persistence::PostgresSchemaSnapshotRepository;
    ///
    /// let repository =
    ///     PostgresSchemaSnapshotRepository::new("postgres://postgres:postgres@localhost/test");
    ///
    /// let _ = repository;
    /// ```
    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
        }
    }
}

impl SchemaSnapshotRepository for PostgresSchemaSnapshotRepository {
    fn load_schema_diagram(&self) -> Result<SchemaDiagram, SchemaSnapshotRepositoryError> {
        let mut client = Client::connect(self.database_url.as_str(), NoTls)
            .map_err(|error| SchemaSnapshotRepositoryError::connection(error.to_string()))?;

        let tables = query_tables(&mut client)?;
        let columns_by_table = query_columns(&mut client)?;
        let relationships = query_relationships(&mut client)?;

        let mut diagram_tables = Vec::with_capacity(tables.len());
        for table_name in tables {
            let columns = columns_by_table
                .get(table_name.as_str())
                .cloned()
                .unwrap_or_default();
            diagram_tables.push(SchemaTable {
                name: table_name,
                columns,
            });
        }

        Ok(SchemaDiagram {
            tables: diagram_tables,
            relationships,
        })
    }
}

fn query_tables(client: &mut Client) -> Result<Vec<String>, SchemaSnapshotRepositoryError> {
    let query = concat!(
        "SELECT cls.relname AS table_name ",
        "FROM pg_catalog.pg_class cls ",
        "JOIN pg_catalog.pg_namespace ns ",
        "  ON ns.oid = cls.relnamespace ",
        "WHERE ns.nspname = 'public' ",
        "  AND cls.relkind IN ('r', 'p') ",
        "ORDER BY cls.relname"
    );

    let rows = client
        .query(query, &[])
        .map_err(|error| SchemaSnapshotRepositoryError::query(error.to_string()))?;

    Ok(rows.into_iter().map(|row| row.get("table_name")).collect())
}

fn query_columns(
    client: &mut Client,
) -> Result<BTreeMap<String, Vec<SchemaColumn>>, SchemaSnapshotRepositoryError> {
    let query = concat!(
        "SELECT ",
        "  cls.relname AS table_name, ",
        "  attr.attname AS column_name, ",
        "  pg_catalog.format_type(attr.atttypid, attr.atttypmod) AS data_type, ",
        "  NOT attr.attnotnull AS is_nullable, ",
        "  EXISTS (",
        "    SELECT 1 ",
        "    FROM pg_catalog.pg_index idx ",
        "    WHERE idx.indrelid = cls.oid ",
        "      AND idx.indisprimary ",
        "      AND attr.attnum = ANY(idx.indkey)",
        "  ) AS is_primary_key ",
        "FROM pg_catalog.pg_attribute attr ",
        "JOIN pg_catalog.pg_class cls ",
        "  ON cls.oid = attr.attrelid ",
        "JOIN pg_catalog.pg_namespace ns ",
        "  ON ns.oid = cls.relnamespace ",
        "WHERE ns.nspname = 'public' ",
        "  AND cls.relkind IN ('r', 'p') ",
        "  AND attr.attnum > 0 ",
        "  AND NOT attr.attisdropped ",
        "ORDER BY cls.relname, attr.attnum"
    );

    let rows = client
        .query(query, &[])
        .map_err(|error| SchemaSnapshotRepositoryError::query(error.to_string()))?;

    let mut columns_by_table: BTreeMap<String, Vec<SchemaColumn>> = BTreeMap::new();
    for row in rows {
        let table_name: String = row.get("table_name");
        let column = SchemaColumn {
            name: row.get("column_name"),
            data_type: row.get("data_type"),
            is_primary_key: row.get("is_primary_key"),
            is_nullable: row.get("is_nullable"),
        };
        columns_by_table.entry(table_name).or_default().push(column);
    }

    Ok(columns_by_table)
}

fn query_relationships(
    client: &mut Client,
) -> Result<Vec<SchemaRelationship>, SchemaSnapshotRepositoryError> {
    let query = concat!(
        "SELECT ",
        "  source.relname AS referencing_table, ",
        "  source_attr.attname AS referencing_column, ",
        "  target.relname AS referenced_table, ",
        "  target_attr.attname AS referenced_column, ",
        "  NOT source_attr.attnotnull AS referencing_is_nullable ",
        "FROM pg_catalog.pg_constraint con ",
        "JOIN pg_catalog.pg_class source ",
        "  ON source.oid = con.conrelid ",
        "JOIN pg_catalog.pg_namespace source_ns ",
        "  ON source_ns.oid = source.relnamespace ",
        "JOIN pg_catalog.pg_class target ",
        "  ON target.oid = con.confrelid ",
        "JOIN pg_catalog.pg_namespace target_ns ",
        "  ON target_ns.oid = target.relnamespace ",
        "JOIN unnest(con.conkey) WITH ORDINALITY AS source_key(attnum, ord) ",
        "  ON TRUE ",
        "JOIN unnest(con.confkey) WITH ORDINALITY AS target_key(attnum, ord) ",
        "  ON source_key.ord = target_key.ord ",
        "JOIN pg_catalog.pg_attribute source_attr ",
        "  ON source_attr.attrelid = source.oid ",
        " AND source_attr.attnum = source_key.attnum ",
        "JOIN pg_catalog.pg_attribute target_attr ",
        "  ON target_attr.attrelid = target.oid ",
        " AND target_attr.attnum = target_key.attnum ",
        "WHERE con.contype = 'f' ",
        "  AND source_ns.nspname = 'public' ",
        "  AND target_ns.nspname = 'public' ",
        "ORDER BY ",
        "  source.relname, ",
        "  source_attr.attname, ",
        "  target.relname, ",
        "  target_attr.attname, ",
        "  con.conname"
    );

    let rows = client
        .query(query, &[])
        .map_err(|error| SchemaSnapshotRepositoryError::query(error.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|row| SchemaRelationship {
            referencing_table: row.get("referencing_table"),
            referencing_column: row.get("referencing_column"),
            referenced_table: row.get("referenced_table"),
            referenced_column: row.get("referenced_column"),
            referencing_is_nullable: row.get("referencing_is_nullable"),
        })
        .collect())
}
