//! Helper binary invoked by `pg_embedded_setup_unpriv` when tests run as root.
//!
//! The worker receives an operation (`setup`, `start`, or `stop`) plus a path
//! to a JSON payload describing the PostgreSQL settings and environment. The
//! payload format matches [`pg_embedded_setup_unpriv::worker::WorkerPayload`]
//! so no additional serialisation glue is required.

use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::PathBuf;

use color_eyre::eyre::{Context, Report, Result, eyre};
use pg_embedded_setup_unpriv::worker::WorkerPayload;
use postgresql_embedded::PostgreSQL;
use tokio::runtime::Builder;

fn main() -> Result<()> {
    color_eyre::install()?;
    run_worker(env::args_os())
}

fn run_worker(mut args: impl Iterator<Item = OsString>) -> Result<()> {
    let _program = args.next();
    let op_arg = args
        .next()
        .ok_or_else(|| eyre!("missing operation argument"))?;
    let operation = Operation::parse(&op_arg)?;
    let config_path = PathBuf::from(
        args.next()
            .ok_or_else(|| eyre!("missing config path argument"))?,
    );
    if let Some(extra) = args.next() {
        return Err(eyre!(
            "unexpected extra argument: {}; expected only operation and config path",
            extra.to_string_lossy()
        ));
    }

    let payload = load_payload(&config_path)?;
    execute(operation, payload)
}

fn load_payload(path: &PathBuf) -> Result<WorkerPayload> {
    let payload =
        fs::read(path).with_context(|| format!("failed to read worker config at {path:?}"))?;
    let parsed: WorkerPayload = serde_json::from_slice(&payload)
        .with_context(|| format!("failed to parse worker config at {path:?}"))?;
    Ok(parsed)
}

fn execute(operation: Operation, payload: WorkerPayload) -> Result<()> {
    let settings = payload
        .settings
        .into_settings()
        .map_err(|err| Report::new(err).wrap_err("failed to rebuild postgres settings"))?;
    apply_environment(payload.environment);

    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .wrap_err("failed to build Pg worker runtime")?;

    let mut postgres = PostgreSQL::new(settings);
    runtime
        .block_on(async move {
            match operation {
                Operation::Setup => postgres.setup().await,
                Operation::Start => postgres.start().await,
                Operation::Stop => postgres.stop().await,
            }
        })
        .with_context(|| format!("postgresql_embedded::{operation} failed"))?;
    Ok(())
}

fn apply_environment(env: Vec<(String, Option<String>)>) {
    for (key, value) in env {
        // SAFETY: Called before spawning any threads, so no data races possible.
        match value {
            Some(val) => unsafe { env::set_var(&key, val) },
            None => unsafe { env::remove_var(&key) },
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Operation {
    Setup,
    Start,
    Stop,
}

impl Operation {
    fn parse(raw: &OsStr) -> Result<Self> {
        match raw.to_string_lossy().as_ref() {
            "setup" => Ok(Self::Setup),
            "start" => Ok(Self::Start),
            "stop" => Ok(Self::Stop),
            other => Err(eyre!(
                "unknown pg_worker operation '{other}'; valid operations are setup, start, and stop"
            )),
        }
    }
}

impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Setup => "setup",
            Self::Start => "start",
            Self::Stop => "stop",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rejects_unknown_operation() {
        let err = Operation::parse(&OsString::from("noop")).expect_err("invalid op should fail");
        assert!(err.to_string().contains("unknown pg_worker operation"));
    }

    #[test]
    fn run_worker_rejects_extra_argument() {
        let args = vec![
            OsString::from("pg_worker"),
            OsString::from("setup"),
            OsString::from("/tmp/config.json"),
            OsString::from("unexpected"),
        ];
        let err = run_worker(args.into_iter()).expect_err("extra argument must fail");
        assert!(err.to_string().contains("unexpected extra argument"));
    }
}
