//! Helpers for bootstrapping embedded PostgreSQL in integration tests.
//!
//! `pg-embed-setup-unpriv` defaults to using `/var/tmp` for installation and
//! data directories. Under the Codex CLI sandbox, writing outside of the
//! workspace is blocked, so tests that rely on the embedded cluster need to
//! override these paths.
//!
//! This module scopes `PG_RUNTIME_DIR` and `PG_DATA_DIR` overrides to the
//! bootstrap call and serialises environment mutation to avoid global
//! environment races across parallel tests.
//!
//! When either `PG_RUNTIME_DIR` or `PG_DATA_DIR` is missing, this module sets
//! both for the duration of the bootstrap, ensuring the embedded cluster uses
//! a consistent workspace-backed configuration.
//! bootstrap call and serialises environment mutation to avoid global
//! environment races across parallel tests.
//!
//! When either `PG_RUNTIME_DIR` or `PG_DATA_DIR` is missing, this module sets
//! both for the duration of the bootstrap, ensuring the embedded cluster uses
//! a consistent workspace-backed configuration.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use env_lock::EnvGuard;
use pg_embedded_setup_unpriv::TestCluster;
use uuid::Uuid;

static PG_EMBED_BOOTSTRAP_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn pg_embed_target_dir() -> PathBuf {
    if let Some(target_dir) = std::env::var_os("CARGO_TARGET_DIR") {
        return PathBuf::from(target_dir).join("pg-embed");
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("target")
        .join("pg-embed")
}

fn create_unique_pg_embed_dirs() -> Result<(PathBuf, PathBuf), std::io::Error> {
    let unique = format!("bootstrap-{}-{}", std::process::id(), Uuid::new_v4());
    let base = pg_embed_target_dir().join(unique);
    let runtime_dir = base.join("install");
    let data_dir = base.join("data");

    std::fs::create_dir_all(&runtime_dir)?;
    std::fs::create_dir_all(&data_dir)?;

    Ok((runtime_dir, data_dir))
}

/// Bootstraps a [`TestCluster`] using workspace-backed data directories when needed.
///
/// When `PG_RUNTIME_DIR`/`PG_DATA_DIR` are already set, this function leaves
/// them untouched. If either value is missing, this function sets both to
/// unique directories under the target directory so the bootstrap works in
/// sandboxed environments.
pub fn test_cluster() -> Result<TestCluster, String> {
    let _bootstrap_guard = PG_EMBED_BOOTSTRAP_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|err| err.into_inner());

    let needs_override =
        std::env::var_os("PG_RUNTIME_DIR").is_none() || std::env::var_os("PG_DATA_DIR").is_none();

    let _env_guard: Option<EnvGuard<'static>> = if needs_override {
        let (runtime_dir, data_dir) =
            create_unique_pg_embed_dirs().map_err(|err| err.to_string())?;

        let runtime_dir_value = runtime_dir.to_string_lossy().into_owned();
        let data_dir_value = data_dir.to_string_lossy().into_owned();

        Some(env_lock::lock_env([
            ("PG_RUNTIME_DIR", Some(runtime_dir_value)),
            ("PG_DATA_DIR", Some(data_dir_value)),
        ]))
    } else {
        None
    };

    let cluster = TestCluster::new().map_err(|err| format!("{err:?}"))?;
    Ok(cluster)
}
