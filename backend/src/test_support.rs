//! Test utilities for the backend crate.
//!
//! This module provides shared helpers for both unit tests (in `src/`) and
//! integration tests (in `tests/`). It is only compiled when running tests.

pub mod cap_fs {
    //! Capability-safe filesystem helpers for tests.
    //!
    //! The backend forbids direct `std::fs` calls. These helpers provide common
    //! read/write/existence/remove operations built on `cap_std::fs::Dir` so
    //! test suites can share consistent, policy-compliant file access.

    use std::ffi::OsString;
    use std::io;
    use std::path::Path;

    use cap_std::{ambient_authority, fs::Dir};

    /// Read a UTF-8 text file through `cap_std`.
    pub fn read_file_to_string(path: &Path) -> io::Result<String> {
        let (parent, file_name) = parent_and_file_name(path)?;
        let directory = Dir::open_ambient_dir(parent, ambient_authority())?;
        directory.read_to_string(Path::new(&file_name))
    }

    /// Write bytes to a file through `cap_std`.
    pub fn write_file(path: &Path, contents: &[u8]) -> io::Result<()> {
        let (parent, file_name) = parent_and_file_name(path)?;
        let directory = Dir::open_ambient_dir(parent, ambient_authority())?;
        directory.write(Path::new(&file_name), contents)
    }

    /// Return true when `path` exists, false when it does not.
    pub fn path_exists(path: &Path) -> bool {
        let Ok((parent, file_name)) = parent_and_file_name(path) else {
            return false;
        };
        let Ok(directory) = Dir::open_ambient_dir(parent, ambient_authority()) else {
            return false;
        };
        directory.exists(Path::new(&file_name))
    }

    /// Remove a directory tree, treating a missing path as success.
    pub fn remove_directory(path: &Path) -> io::Result<()> {
        let (parent, directory_name) = parent_and_file_name(path)?;
        let directory = Dir::open_ambient_dir(parent, ambient_authority())?;
        match directory.remove_dir_all(Path::new(&directory_name)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn parent_and_file_name(path: &Path) -> io::Result<(&Path, OsString)> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let file_name = path.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "path must include a file or directory name",
            )
        })?;
        Ok((parent, file_name.to_os_string()))
    }
}

pub mod openapi {
    //! OpenAPI schema traversal helpers.
    //!
    //! Provides utilities for extracting and inspecting utoipa `Schema` types,
    //! particularly for resolving `RefOr<Schema>` wrappers to concrete `Object`
    //! schemas with diagnostic error messages on type mismatches.

    use utoipa::openapi::RefOr;
    use utoipa::openapi::schema::{Object, Schema};

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
        match obj.properties.get(field) {
            Some(property) => property,
            None => panic!("property '{field}' not found"),
        }
    }
}
