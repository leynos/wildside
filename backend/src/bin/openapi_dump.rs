//! Print the OpenAPI document as JSON.

use backend::ApiDoc;
use serde::Serialize;
use serde_json::{ser::PrettyFormatter, Serializer};
use utoipa::OpenApi;

/// Write the OpenAPI document to stdout.
/// Serialises with a two-space indent to match repo style.
fn main() {
    let doc = ApiDoc::openapi();
    let mut buf = Vec::new();
    let formatter = PrettyFormatter::with_indent(b"  ");
    let mut serializer = Serializer::with_formatter(&mut buf, formatter);
    doc.serialize(&mut serializer)
        .expect("serialising OpenAPI document");
    let json = String::from_utf8(buf).expect("OpenAPI JSON must be valid UTF-8");
    println!("{json}");
}
