//! Print the OpenAPI document as JSON.

use backend::ApiDoc;
use serde_json::{ser::PrettyFormatter, Serializer};
use std::io::{self, Write};
use utoipa::OpenApi;

/// Write the OpenAPI document to stdout.
/// Serialises with a two-space indent to match repo style.
fn main() -> io::Result<()> {
    let doc = ApiDoc::openapi();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let formatter = PrettyFormatter::with_indent(b"  ");
    let mut serializer = Serializer::with_formatter(&mut out, formatter);
    serde::Serialize::serialize(&doc, &mut serializer)
        .map_err(|e| io::Error::other(format!("serialising OpenAPI document: {e}")))?;
    writeln!(out)?;
    Ok(())
}
