//! Ingest OSM data into backend POI tables with geofence and provenance controls.
#![cfg_attr(not(any(test, doctest)), deny(clippy::unwrap_used))]
#![cfg_attr(not(any(test, doctest)), deny(clippy::expect_used))]

use std::env;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use backend::domain::OsmIngestionCommandService;
use backend::domain::ports::{OsmIngestionCommand, OsmIngestionRequest};
use backend::outbound::osm_source::WildsideDataOsmSourceRepository;
use backend::outbound::persistence::{DbPool, DieselOsmIngestionProvenanceRepository, PoolConfig};
use cap_std::{ambient_authority, fs::Dir};
use clap::Parser;
use mockable::DefaultClock;
use sha2::{Digest, Sha256};
use tokio::runtime::Builder;

/// `ingest-osm` command arguments.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "ingest-osm",
    about = "Ingest geofenced OSM POIs into backend storage with provenance tracking",
    version
)]
struct CliArgs {
    /// Path to an `.osm.pbf` input file.
    #[arg(long = "osm-pbf", value_name = "path")]
    osm_pbf_path: PathBuf,
    /// Canonical source URL captured in provenance.
    #[arg(long = "source-url", value_name = "url")]
    source_url: String,
    /// Geofence identifier used for deterministic rerun keys.
    #[arg(long = "geofence-id", value_name = "id")]
    geofence_id: String,
    /// Geofence bounds as `min_lng,min_lat,max_lng,max_lat`.
    #[arg(
        long = "geofence-bounds",
        value_name = "min_lng,min_lat,max_lng,max_lat",
        value_parser = parse_geofence_bounds
    )]
    geofence_bounds: [f64; 4],
    /// Database connection URL. Falls back to `DATABASE_URL` when omitted.
    #[arg(long = "database-url", value_name = "url")]
    database_url: Option<String>,
}

fn main() -> io::Result<()> {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| io::Error::other(format!("create Tokio runtime: {error}")))?;
    runtime.block_on(async_main())
}

async fn async_main() -> io::Result<()> {
    let args = CliArgs::try_parse().map_err(io::Error::other)?;
    let input_digest = sha256_file(&args.osm_pbf_path)?;

    let database_url = resolve_database_url(args.database_url)?;
    let pool = DbPool::new(PoolConfig::new(&database_url))
        .await
        .map_err(|error| io::Error::other(format!("create database pool: {error}")))?;

    let source_repo = Arc::new(WildsideDataOsmSourceRepository);
    let provenance_repo = Arc::new(DieselOsmIngestionProvenanceRepository::new(pool));
    let command =
        OsmIngestionCommandService::new(source_repo, provenance_repo, Arc::new(DefaultClock));

    let request = OsmIngestionRequest {
        osm_pbf_path: args.osm_pbf_path,
        source_url: args.source_url,
        geofence_id: args.geofence_id,
        geofence_bounds: args.geofence_bounds,
        input_digest,
    };

    let outcome = command
        .ingest(request)
        .await
        .map_err(|error| io::Error::other(format!("ingest command failed: {error}")))?;

    println!("status={:?}", outcome.status);
    println!("source_url={}", outcome.source_url);
    println!("geofence_id={}", outcome.geofence_id);
    println!("input_digest={}", outcome.input_digest);
    println!("imported_at={}", outcome.imported_at.to_rfc3339());
    println!(
        "geofence_bounds={},{},{},{}",
        outcome.geofence_bounds[0],
        outcome.geofence_bounds[1],
        outcome.geofence_bounds[2],
        outcome.geofence_bounds[3]
    );
    println!("raw_poi_count={}", outcome.raw_poi_count);
    println!("persisted_poi_count={}", outcome.persisted_poi_count);

    Ok(())
}

fn parse_geofence_bounds(raw: &str) -> Result<[f64; 4], String> {
    let values = raw
        .split(',')
        .map(str::trim)
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to parse geofence bounds value: {error}"))?;
    if values.len() != 4 {
        return Err(
            "geofence bounds must contain exactly four comma-separated numeric values".to_owned(),
        );
    }
    values.try_into().map_err(|_| {
        "geofence bounds must contain exactly four comma-separated numeric values".to_owned()
    })
}

fn sha256_file(path: &Path) -> io::Result<String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "input path must be a file"))?;
    let directory = Dir::open_ambient_dir(parent, ambient_authority()).map_err(|error| {
        io::Error::other(format!(
            "open input parent directory '{}': {error}",
            parent.display()
        ))
    })?;
    let mut file = directory.open(Path::new(file_name)).map_err(|error| {
        io::Error::other(format!("open input file '{}': {error}", path.display()))
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| {
            io::Error::other(format!("read input file '{}': {error}", path.display()))
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn resolve_database_url(explicit: Option<String>) -> io::Result<String> {
    if let Some(value) = explicit {
        if value.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "--database-url must not be empty when provided",
            ));
        }
        return Ok(value);
    }

    let from_env = env::var("DATABASE_URL").map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "database URL missing: set --database-url or DATABASE_URL",
        )
    })?;
    if from_env.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "DATABASE_URL must not be empty",
        ));
    }
    Ok(from_env)
}

#[cfg(test)]
mod tests {
    //! Unit tests for CLI parsing helpers.

    use std::io::Write;

    use rstest::rstest;
    use tempfile::NamedTempFile;

    use super::{parse_geofence_bounds, resolve_database_url, sha256_file};

    #[rstest]
    fn geofence_bounds_parser_accepts_valid_input() {
        let bounds = parse_geofence_bounds("-3.3,55.9,-3.1,56.0").expect("bounds should parse");
        assert_eq!(bounds, [-3.3, 55.9, -3.1, 56.0]);
    }

    #[rstest]
    fn geofence_bounds_parser_rejects_wrong_arity() {
        let error = parse_geofence_bounds("-3.3,55.9,-3.1").expect_err("arity should fail");
        assert!(error.contains("exactly four"));
    }

    #[rstest]
    fn geofence_bounds_parser_rejects_non_numeric_values() {
        let error =
            parse_geofence_bounds("-3.3,55.9,-3.1,abc").expect_err("numeric parse should fail");
        assert!(error.contains("failed to parse"));
    }

    #[rstest]
    fn sha256_file_is_deterministic() {
        let mut file = NamedTempFile::new().expect("temp file");
        writeln!(file, "wildside").expect("write fixture");
        let first = sha256_file(file.path()).expect("first digest");
        let second = sha256_file(file.path()).expect("second digest");
        assert_eq!(first, second);
        assert_eq!(first.len(), 64);
    }

    #[rstest]
    fn resolve_database_url_rejects_empty_explicit() {
        let error = resolve_database_url(Some("   ".to_owned())).expect_err("empty should fail");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }
}
