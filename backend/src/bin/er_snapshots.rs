#![cfg_attr(not(any(test, doctest)), deny(clippy::unwrap_used))]
#![cfg_attr(not(any(test, doctest)), deny(clippy::expect_used))]
//! Generate ER diagram snapshots from migration-backed schema metadata.
//!
//! # Examples
//! ```sh
//! cargo run --manifest-path backend/Cargo.toml --bin er-snapshots -- --output-dir docs/diagrams/er
//! ```

use std::env;
use std::io;
use std::path::PathBuf;

use backend::er_snapshots::{CommandMermaidRenderer, SnapshotRequest, generate_from_migrations};
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

fn default_output_dir() -> PathBuf {
    PathBuf::from("docs/diagrams/er")
}

#[derive(Debug, Clone, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "ER_SNAPSHOTS")]
struct CliArgs {
    #[ortho_config(default = default_output_dir())]
    output_dir: PathBuf,
    #[ortho_config(cli_long = "skip-svg")]
    skip_svg: bool,
}

fn main() -> io::Result<()> {
    let parsed = CliArgs::load_from_iter(env::args_os()).map_err(io::Error::other)?;
    let request = SnapshotRequest {
        output_dir: parsed.output_dir,
        should_render_svg: !parsed.skip_svg,
    };
    let renderer = CommandMermaidRenderer::default();

    let output = generate_from_migrations(&renderer, &request)
        .map_err(|error| io::Error::other(format!("generate ER snapshots: {error}")))?;

    println!(
        "Wrote Mermaid snapshot: {}",
        output.mermaid_path.to_string_lossy()
    );
    if let Some(svg_path) = output.svg_path {
        println!("Wrote SVG snapshot: {}", svg_path.to_string_lossy());
    }

    Ok(())
}
