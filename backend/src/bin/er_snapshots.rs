#![cfg_attr(not(any(test, doctest)), deny(clippy::unwrap_used))]
#![cfg_attr(not(any(test, doctest)), forbid(clippy::expect_used))]
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

#[derive(Debug, Clone)]
struct CliArgs {
    output_dir: PathBuf,
    render_svg: bool,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("docs/diagrams/er"),
            render_svg: true,
        }
    }
}

impl CliArgs {
    fn parse(arguments: impl IntoIterator<Item = String>) -> io::Result<Self> {
        let mut args = arguments.into_iter();
        let mut parsed = Self::default();

        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--output-dir" => {
                    parsed.output_dir = parse_output_dir_value(&mut args)?;
                }
                "--skip-svg" => {
                    parsed.render_svg = false;
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                unknown => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("unknown argument: {unknown}"),
                    ));
                }
            }
        }

        Ok(parsed)
    }
}

fn parse_output_dir_value(args: &mut impl Iterator<Item = String>) -> io::Result<PathBuf> {
    match args.next() {
        Some(path) => Ok(PathBuf::from(path)),
        None => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--output-dir requires a value",
        )),
    }
}

fn print_help() {
    println!(concat!(
        "Usage: er-snapshots [OPTIONS]\n\n",
        "Options:\n",
        "  --output-dir <path>  Output directory for snapshot artefacts\n",
        "  --skip-svg           Generate Mermaid source only (skip SVG rendering)\n",
        "  -h, --help           Print this help message\n",
    ));
}

fn main() -> io::Result<()> {
    let parsed = CliArgs::parse(env::args().skip(1))?;
    let request = SnapshotRequest {
        output_dir: parsed.output_dir,
        render_svg: parsed.render_svg,
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
