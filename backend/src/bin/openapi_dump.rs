#![cfg_attr(not(any(test, doctest)), deny(clippy::unwrap_used))]
#![cfg_attr(not(any(test, doctest)), forbid(clippy::expect_used))]
//! Print the OpenAPI document as JSON.
//!
//! # Examples
//! ```sh
//! cargo run --quiet --manifest-path backend/Cargo.toml --bin openapi-dump > spec/openapi.json
//! ```

use backend::ApiDoc;
use serde_json::to_writer_pretty;
use std::io::{self, BufWriter, Write};

/// Write the OpenAPI document to stdout.
/// Serialises with a two-space indent to match repo style.
fn main() -> io::Result<()> {
    let doc = ApiDoc::openapi();
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    to_writer_pretty(&mut out, &doc)
        .map_err(|e| io::Error::other(format!("serialising OpenAPI document: {e}")))?;
    writeln!(out)?;
    Ok(())
}
