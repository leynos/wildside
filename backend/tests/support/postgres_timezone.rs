//! Compatibility for timezone names required by PostgreSQL clients.

use std::io;

use cap_std::fs::Dir;

/// Ensures PostgreSQL can resolve `UTC` in its bundled timezone directory.
///
/// Some embedded PostgreSQL archives contain `UCT`, the equivalent zero-offset
/// zone, but omit the `UTC` alias. `diesel-async` requests `UTC` during
/// connection setup. A hard link adds the missing name without duplicating the
/// timezone data, and creation is atomic when multiple test processes share
/// the same installation directory.
pub(crate) fn ensure_utc_alias(timezone_dir: &Dir) -> io::Result<()> {
    if timezone_dir.try_exists("UTC")? {
        return Ok(());
    }

    if !timezone_dir.try_exists("UCT")? {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "neither UTC nor its UCT equivalent exists in PostgreSQL timezone data",
        ));
    }

    match timezone_dir.hard_link("UCT", timezone_dir, "UTC") {
        Ok(()) => Ok(()),
        Err(error)
            if error.kind() == io::ErrorKind::AlreadyExists
                && timezone_dir.try_exists("UTC")? =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}
