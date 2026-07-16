//! Table helpers shared by selected integration-test crates.

use super::format_postgres_error;

/// Drop a table by name from a test database.
///
/// The identifier is escaped so helpers can safely accept test-provided
/// table names.
///
/// # Examples
///
/// ```ignore
/// let url = "postgres://localhost/test";
/// let result = crate::support::table_helpers::drop_table(url, "offline_bundles");
/// assert!(result.is_ok());
/// ```
pub fn drop_table(url: &str, table_name: &str) -> Result<(), String> {
    let mut client = postgres::Client::connect(url, postgres::NoTls)
        .map_err(|err| format_postgres_error(&err))?;
    let escaped_name = table_name.replace('"', "\"\"");
    let sql = format!(r#"DROP TABLE IF EXISTS "{escaped_name}""#);
    client
        .batch_execute(sql.as_str())
        .map_err(|err| format_postgres_error(&err))
}
