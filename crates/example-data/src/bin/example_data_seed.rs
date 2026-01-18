//! Seed registry CLI for adding named demo data seeds.
//!
//! This binary delegates to `example_data::seed_registry_cli` for parsing and
//! update logic, keeping the CLI behaviour testable without spawning a process.

use std::env;
use std::io::{self, Write};
use std::process::ExitCode;

use example_data::seed_registry_cli::{
    CliError, ParseOutcome, apply_update, parse_args, success_message,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            if let Err(write_err) = writeln!(io::stderr().lock(), "{err}") {
                drop(write_err);
            }
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), CliError> {
    match parse_args(env::args().skip(1))? {
        ParseOutcome::Help => {
            print_usage(io::stdout().lock());
            Ok(())
        }
        ParseOutcome::Options(options) => {
            let update = apply_update(&options)?;
            let message = success_message(&update, options.registry_path());
            write_success(&message);
            Ok(())
        }
    }
}

fn print_usage(mut out: impl Write) {
    let usage = concat!(
        "Usage: example-data-seed --registry <path> [options]\n",
        "\n",
        "Options:\n",
        "  --registry <path>    Path to the seed registry JSON file\n",
        "  --name <name>        Seed name to add (defaults to generated)\n",
        "  --seed <seed>        RNG seed value (defaults to random)\n",
        "  --user-count <n>     User count (defaults to 12)\n",
        "  -h, --help           Print this help output\n",
    );
    if let Err(err) = out.write_all(usage.as_bytes()) {
        drop(err);
    }
}

fn write_success(message: &str) {
    if let Err(err) = writeln!(io::stdout().lock(), "{message}") {
        drop(err);
    }
}
