//! Helpers for bootstrapping embedded PostgreSQL in integration tests.
//!
//! `pg-embed-setup-unpriv` defaults to using `/var/tmp` for installation and
//! data directories. Under the Codex CLI sandbox, writing outside of the
//! workspace is blocked, so tests that rely on the embedded cluster need to
//! override these paths.
//!
//! This module scopes `PG_RUNTIME_DIR` and `PG_DATA_DIR` overrides to the
//! bootstrap call and serialises the bootstrap to avoid global environment
//! races across parallel tests.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use pg_embedded_setup_unpriv::TestCluster;
use uuid::Uuid;

static PG_EMBED_BOOTSTRAP_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct ScopedEnvVars {
    previous_values: Vec<(String, Option<OsString>)>,
}

impl ScopedEnvVars {
    fn set(vars: impl IntoIterator<Item = (&'static str, OsString)>) -> Self {
        let mut previous_values = Vec::new();
        for (key, value) in vars {
            let key_owned = key.to_owned();
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            previous_values.push((key_owned, previous));
        }

        Self { previous_values }
    }
}

impl Drop for ScopedEnvVars {
    fn drop(&mut self) {
        for (key, previous) in self.previous_values.drain(..) {
            match previous {
                Some(value) => std::env::set_var(&key, value),
                None => std::env::remove_var(&key),
            }
        }
    }
}

fn pg_embed_target_dir() -> PathBuf {
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
/// them untouched. Otherwise, it points them at unique directories under
/// `target/pg-embed/` so the bootstrap works in sandboxed environments.
pub fn test_cluster() -> Result<TestCluster, String> {
    let lock = PG_EMBED_BOOTSTRAP_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("pg-embed bootstrap lock");

    let needs_override =
        std::env::var_os("PG_RUNTIME_DIR").is_none() || std::env::var_os("PG_DATA_DIR").is_none();

    let _scoped_env = if needs_override {
        let (runtime_dir, data_dir) =
            create_unique_pg_embed_dirs().map_err(|err| err.to_string())?;
        Some(ScopedEnvVars::set([
            ("PG_RUNTIME_DIR", runtime_dir.into_os_string()),
            ("PG_DATA_DIR", data_dir.into_os_string()),
        ]))
    } else {
        None
    };

    let cluster = TestCluster::new().map_err(|err| format!("{err:?}"))?;
    drop(lock);
    Ok(cluster)
}
