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
    // `sha2` 0.11 finalizes to `hybrid_array::Array<u8, _>`, which no longer
    // implements `LowerHex`, so the digest is encoded explicitly.
    Ok(hex::encode(hasher.finalize()))
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

    use std::io::{self, Write};

    use rstest::rstest;
    use tempfile::NamedTempFile;

    use super::{Digest, Sha256, parse_geofence_bounds, resolve_database_url, sha256_file};

    /// Write `contents` to a temporary file and digest it via [`sha256_file`].
    ///
    /// Arrangement is fallible, so the helper propagates rather than panicking;
    /// each test unwraps in its own body so a failure reads as that test's
    /// verdict.
    fn digest_of(contents: &[u8]) -> io::Result<String> {
        let mut file = NamedTempFile::new()?;
        file.write_all(contents)?;
        file.flush()?;
        sha256_file(file.path())
    }

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

    /// Pin the rendered digest against the published SHA-256 vectors, so a
    /// future change to the encoding (or to the digest crate) cannot silently
    /// alter the provenance keys already recorded in the database.
    #[rstest]
    #[case::empty(
        b"",
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    )]
    #[case::abc(
        b"abc",
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    )]
    fn sha256_file_matches_known_vectors(#[case] contents: &[u8], #[case] expected: &str) {
        let digest = digest_of(contents).expect("digest fixture");
        assert_eq!(digest, expected);
    }

    /// The hasher reads through a fixed 8 KiB buffer, so a payload spanning
    /// several reads must agree with a single-shot digest of the same bytes.
    #[rstest]
    fn sha256_file_hashes_payloads_larger_than_the_read_buffer() {
        let contents: Vec<u8> = (0..40_000_u32).map(|index| (index % 251) as u8).collect();
        let expected = hex::encode(Sha256::digest(&contents));
        let digest = digest_of(&contents).expect("digest fixture");
        assert_eq!(digest, expected);
    }

    /// Digests are rendered as fixed-width lowercase hex, including the leading
    /// zeroes that `{:x}`-style formatting of individual bytes would drop.
    #[rstest]
    fn sha256_file_renders_fixed_width_lowercase_hex() {
        let digest = digest_of(b"wildside").expect("digest fixture");
        assert_eq!(digest.len(), 64);
        assert!(
            digest
                .chars()
                .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character)),
            "digest should be lowercase hex: {digest}"
        );
    }

    #[rstest]
    fn resolve_database_url_rejects_empty_explicit() {
        let error = resolve_database_url(Some("   ".to_owned())).expect_err("empty should fail");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }
}
