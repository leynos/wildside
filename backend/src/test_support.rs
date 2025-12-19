//! Test utilities for the backend crate.
//!
//! This module provides shared helpers for both unit tests (in `src/`) and
//! integration tests (in `tests/`). It is only compiled when running tests.

pub mod openapi {
    //! OpenAPI schema traversal helpers.
    //!
    //! Provides utilities for extracting and inspecting utoipa `Schema` types,
    //! particularly for resolving `RefOr<Schema>` wrappers to concrete `Object`
    //! schemas with diagnostic error messages on type mismatches.

    use utoipa::openapi::schema::{Object, Schema};
    use utoipa::openapi::RefOr;

    /// Extract an `Object` schema, panicking with a diagnostic if not an Object.
    ///
    /// Provides detailed error messages for refs, combinators, and other schema types.
    pub fn unwrap_object_schema<'a>(schema: &'a RefOr<Schema>, name: &str) -> &'a Object {
        match schema {
            RefOr::T(Schema::Object(obj)) => obj,
            RefOr::Ref(reference) => {
                panic!(
                    "schema '{name}' is a $ref to '{}'; resolve the reference first",
                    reference.ref_location
                );
            }
            RefOr::T(Schema::AllOf(_)) => {
                panic!("schema '{name}' is an AllOf combinator; inspect composed schemas");
            }
            RefOr::T(Schema::OneOf(_)) => {
                panic!("schema '{name}' is a OneOf combinator; inspect variant schemas");
            }
            RefOr::T(Schema::AnyOf(_)) => {
                panic!("schema '{name}' is an AnyOf combinator; inspect variant schemas");
            }
            RefOr::T(Schema::Array(_)) => {
                panic!("schema '{name}' is an Array, not an Object");
            }
            _ => panic!("schema '{name}' has unexpected type"),
        }
    }

    /// Get a property from an Object schema by name.
    ///
    /// Panics if the property does not exist.
    pub fn get_property<'a>(obj: &'a Object, field: &str) -> &'a RefOr<Schema> {
        obj.properties
            .get(field)
            .unwrap_or_else(|| panic!("property '{field}' not found"))
    }
}
