//! Domain types for entity-relationship (ER) diagram snapshots.
//!
//! These types keep schema visualization logic infrastructure-agnostic so
//! adapters can provide schema metadata without leaking persistence details.

use std::collections::BTreeMap;

/// A full schema snapshot used for ER diagram rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaDiagram {
    pub tables: Vec<SchemaTable>,
    pub relationships: Vec<SchemaRelationship>,
}

impl SchemaDiagram {
    /// Return a stable, deterministically ordered clone of the diagram.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use backend::domain::er_diagram::{SchemaDiagram, SchemaTable};
    ///
    /// let diagram = SchemaDiagram {
    ///     tables: vec![
    ///         SchemaTable { name: "z_table".to_owned(), columns: vec![] },
    ///         SchemaTable { name: "a_table".to_owned(), columns: vec![] },
    ///     ],
    ///     relationships: vec![],
    /// };
    ///
    /// let normalized = diagram.normalized();
    /// assert_eq!(normalized.tables[0].name, "a_table");
    /// ```
    pub fn normalized(&self) -> Self {
        let mut tables = self.tables.clone();
        for table in &mut tables {
            table
                .columns
                .sort_by(|left, right| left.name.cmp(&right.name));
        }
        tables.sort_by(|left, right| left.name.cmp(&right.name));

        let mut relationships = self.relationships.clone();
        relationships.sort_by(|left, right| {
            (
                left.referenced_table.as_str(),
                left.referencing_table.as_str(),
                left.referenced_column.as_str(),
                left.referencing_column.as_str(),
            )
                .cmp(&(
                    right.referenced_table.as_str(),
                    right.referencing_table.as_str(),
                    right.referenced_column.as_str(),
                    right.referencing_column.as_str(),
                ))
        });

        Self {
            tables,
            relationships,
        }
    }
}

/// A database table with typed columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaTable {
    pub name: String,
    pub columns: Vec<SchemaColumn>,
}

/// A typed database column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaColumn {
    pub name: String,
    pub data_type: String,
    pub is_primary_key: bool,
    pub is_nullable: bool,
}

/// A single foreign-key relationship between two columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaRelationship {
    pub referencing_table: String,
    pub referencing_column: String,
    pub referenced_table: String,
    pub referenced_column: String,
    pub referencing_is_nullable: bool,
}

/// Render a Mermaid ER diagram from a schema snapshot.
///
/// # Examples
///
/// ```rust,ignore
/// use backend::domain::{SchemaColumn, SchemaDiagram, SchemaRelationship, SchemaTable};
/// use backend::domain::render_mermaid_er_diagram;
///
/// let diagram = SchemaDiagram {
///     tables: vec![
///         SchemaTable { name: "users".to_owned(), columns: vec![SchemaColumn {
///             name: "id".to_owned(), data_type: "uuid".to_owned(),
///             is_primary_key: true, is_nullable: false
///         }]},
///         SchemaTable { name: "routes".to_owned(), columns: vec![SchemaColumn {
///             name: "user_id".to_owned(), data_type: "uuid".to_owned(),
///             is_primary_key: false, is_nullable: false
///         }]},
///     ],
///     relationships: vec![SchemaRelationship {
///         referencing_table: "routes".to_owned(),
///         referencing_column: "user_id".to_owned(),
///         referenced_table: "users".to_owned(),
///         referenced_column: "id".to_owned(),
///         referencing_is_nullable: false,
///     }],
/// };
///
/// assert!(render_mermaid_er_diagram(&diagram).contains("Users ||--|{ Routes"));
/// ```
pub fn render_mermaid_er_diagram(diagram: &SchemaDiagram) -> String {
    let normalized = diagram.normalized();
    let entity_names = entity_names(&normalized.tables);
    let mut output = String::from("erDiagram\n");

    for table in &normalized.tables {
        render_table_entity(&mut output, table, &entity_names);
    }

    for relationship in &normalized.relationships {
        render_relationship_line(&mut output, relationship, &entity_names);
    }

    output
}

fn render_table_entity(
    output: &mut String,
    table: &SchemaTable,
    entity_names: &BTreeMap<&str, String>,
) {
    let entity_name = entity_names
        .get(table.name.as_str())
        .cloned()
        .unwrap_or_else(|| to_pascal_case(table.name.as_str()));
    output.push_str("  ");
    output.push_str(entity_name.as_str());
    output.push_str(" {\n");

    for column in &table.columns {
        render_column_line(output, column);
    }

    output.push_str("  }\n\n");
}

fn render_column_line(output: &mut String, column: &SchemaColumn) {
    output.push_str("    ");
    output.push_str(sanitize_data_type(column.data_type.as_str()).as_str());
    output.push(' ');
    output.push_str(column.name.as_str());
    if column.is_primary_key {
        output.push_str(" PK");
    }
    output.push('\n');
}

fn render_relationship_line(
    output: &mut String,
    relationship: &SchemaRelationship,
    entity_names: &BTreeMap<&str, String>,
) {
    let parent = entity_names
        .get(relationship.referenced_table.as_str())
        .cloned()
        .unwrap_or_else(|| to_pascal_case(relationship.referenced_table.as_str()));
    let child = entity_names
        .get(relationship.referencing_table.as_str())
        .cloned()
        .unwrap_or_else(|| to_pascal_case(relationship.referencing_table.as_str()));
    let cardinality = if relationship.referencing_is_nullable {
        "||--o{"
    } else {
        "||--|{"
    };

    output.push_str("  ");
    output.push_str(parent.as_str());
    output.push(' ');
    output.push_str(cardinality);
    output.push(' ');
    output.push_str(child.as_str());
    output.push_str(" : \"");
    output.push_str(relationship.referencing_column.as_str());
    output.push_str(" -> ");
    output.push_str(relationship.referenced_column.as_str());
    output.push_str("\"\n");
}

fn entity_names(tables: &[SchemaTable]) -> BTreeMap<&str, String> {
    let mut map = BTreeMap::new();
    for table in tables {
        map.insert(table.name.as_str(), to_pascal_case(table.name.as_str()));
    }
    map
}

fn to_pascal_case(value: &str) -> String {
    let mut output = String::new();

    for segment in value.split('_').filter(|segment| !segment.is_empty()) {
        let mut chars = segment.chars();
        let Some(first) = chars.next() else {
            continue;
        };
        output.extend(first.to_uppercase());
        output.push_str(chars.as_str());
    }

    if output.is_empty() {
        value.to_owned()
    } else {
        output
    }
}

fn sanitize_data_type(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    let mut previous_was_underscore = false;
    let mut characters = value.chars().peekable();

    while let Some(character) = characters.next() {
        if character == '[' && matches!(characters.peek(), Some(']')) {
            characters.next();
            if !previous_was_underscore && !sanitized.is_empty() {
                sanitized.push('_');
            }
            sanitized.push_str("array");
            previous_was_underscore = false;
            continue;
        }

        if character.is_ascii_alphanumeric() {
            sanitized.push(character.to_ascii_lowercase());
            previous_was_underscore = false;
            continue;
        }

        if !previous_was_underscore {
            sanitized.push('_');
            previous_was_underscore = true;
        }
    }

    let trimmed = sanitized.trim_matches('_');
    if trimmed.is_empty() {
        "unknown".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for deterministic Mermaid rendering.

    use rstest::rstest;

    use super::{
        SchemaColumn, SchemaDiagram, SchemaRelationship, SchemaTable, render_mermaid_er_diagram,
    };

    #[rstest]
    fn render_mermaid_er_diagram_orders_tables_columns_and_relationships() {
        let diagram = SchemaDiagram {
            tables: vec![
                SchemaTable {
                    name: "routes".to_owned(),
                    columns: vec![
                        SchemaColumn {
                            name: "user_id".to_owned(),
                            data_type: "uuid".to_owned(),
                            is_primary_key: false,
                            is_nullable: false,
                        },
                        SchemaColumn {
                            name: "id".to_owned(),
                            data_type: "uuid".to_owned(),
                            is_primary_key: true,
                            is_nullable: false,
                        },
                    ],
                },
                SchemaTable {
                    name: "users".to_owned(),
                    columns: vec![
                        SchemaColumn {
                            name: "id".to_owned(),
                            data_type: "uuid".to_owned(),
                            is_primary_key: true,
                            is_nullable: false,
                        },
                        SchemaColumn {
                            name: "display_name".to_owned(),
                            data_type: "text".to_owned(),
                            is_primary_key: false,
                            is_nullable: false,
                        },
                    ],
                },
            ],
            relationships: vec![SchemaRelationship {
                referencing_table: "routes".to_owned(),
                referencing_column: "user_id".to_owned(),
                referenced_table: "users".to_owned(),
                referenced_column: "id".to_owned(),
                referencing_is_nullable: false,
            }],
        };

        let rendered = render_mermaid_er_diagram(&diagram);
        let expected = concat!(
            "erDiagram\n",
            "  Routes {\n",
            "    uuid id PK\n",
            "    uuid user_id\n",
            "  }\n\n",
            "  Users {\n",
            "    text display_name\n",
            "    uuid id PK\n",
            "  }\n\n",
            "  Users ||--|{ Routes : \"user_id -> id\"\n",
        );

        assert_eq!(rendered, expected);
    }

    #[rstest]
    fn render_mermaid_er_diagram_uses_optional_cardinality_for_nullable_foreign_keys() {
        let diagram = SchemaDiagram {
            tables: vec![
                SchemaTable {
                    name: "route_notes".to_owned(),
                    columns: vec![SchemaColumn {
                        name: "route_id".to_owned(),
                        data_type: "uuid".to_owned(),
                        is_primary_key: false,
                        is_nullable: true,
                    }],
                },
                SchemaTable {
                    name: "routes".to_owned(),
                    columns: vec![SchemaColumn {
                        name: "id".to_owned(),
                        data_type: "uuid".to_owned(),
                        is_primary_key: true,
                        is_nullable: false,
                    }],
                },
            ],
            relationships: vec![SchemaRelationship {
                referencing_table: "route_notes".to_owned(),
                referencing_column: "route_id".to_owned(),
                referenced_table: "routes".to_owned(),
                referenced_column: "id".to_owned(),
                referencing_is_nullable: true,
            }],
        };

        let rendered = render_mermaid_er_diagram(&diagram);
        assert!(rendered.contains("Routes ||--o{ RouteNotes"));
    }

    #[rstest]
    fn render_mermaid_er_diagram_preserves_array_marker_in_column_types() {
        let diagram = SchemaDiagram {
            tables: vec![SchemaTable {
                name: "routes".to_owned(),
                columns: vec![SchemaColumn {
                    name: "visited_stop_ids".to_owned(),
                    data_type: "uuid[]".to_owned(),
                    is_primary_key: false,
                    is_nullable: false,
                }],
            }],
            relationships: vec![],
        };

        let rendered = render_mermaid_er_diagram(&diagram);
        assert!(rendered.contains("uuid_array visited_stop_ids"));
    }
}
