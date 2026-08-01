//! Version-aware decoding of persisted job payloads at the queue boundary.
//!
//! Jobs are persisted as [`serde_json::Value`] in the Apalis `apalis.jobs`
//! table, and each job type is a `#[serde(tag = "v")]` envelope (see
//! [`crate::domain::jobs`]). A payload written by a newer producer — for
//! example `{"v": "v2", ...}` — therefore fails to deserialize against a
//! consumer that only knows `v1`. This boundary is where that failure is
//! caught: it converts the rejection into [`JobDispatchError::Rejected`] so the
//! retry/dead-letter policy (roadmap 5.2.3) can act on it. Job handlers only
//! ever receive a successfully decoded, known variant; they never observe a
//! malformed or unsupported-version payload, and the boundary never panics or
//! silently discards a job.

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::domain::ports::JobDispatchError;

/// Diagnostic placeholder used when a payload carries no readable `"v"` field.
const UNKNOWN_VERSION: &str = "<unknown>";
/// Maximum number of UTF-8 bytes copied into logs and rejection diagnostics.
const MAX_VERSION_DIAGNOSTIC_BYTES: usize = 64;

/// Decode a persisted job payload into its versioned envelope type `J`.
///
/// The `"v"` discriminant is read for diagnostics before the typed decode, so
/// an unknown or malformed version can be reported without re-parsing. On
/// failure the job is rejected — never panicked on and never silently dropped.
/// The bounded diagnostic includes only the received version, never the raw
/// payload, because the payload can contain user data.
///
/// # Errors
///
/// Returns [`JobDispatchError::Rejected`] when `payload` does not deserialize
/// into `J`. This covers unknown envelope versions (`{"v": "v2"}`), a missing
/// or non-string `"v"`, and otherwise malformed payloads.
pub fn decode_job<J>(payload: &Value) -> Result<J, JobDispatchError>
where
    J: DeserializeOwned,
{
    let version = envelope_version(payload);
    J::deserialize(payload).map_err(|_error| {
        JobDispatchError::rejected(format!(
            "unrecognized or malformed job envelope version: {version}"
        ))
    })
}

/// Read the `"v"` discriminant as a string slice for diagnostics.
///
/// Returns [`UNKNOWN_VERSION`] when `"v"` is absent or is not a JSON string, so
/// callers always have a safe, non-panicking value to log and report.
fn envelope_version(payload: &Value) -> &str {
    payload
        .get("v")
        .and_then(Value::as_str)
        .map(truncate_version_diagnostic)
        .unwrap_or(UNKNOWN_VERSION)
}

fn truncate_version_diagnostic(version: &str) -> &str {
    if version.len() <= MAX_VERSION_DIAGNOSTIC_BYTES {
        return version;
    }

    let mut end = MAX_VERSION_DIAGNOSTIC_BYTES;
    while !version.is_char_boundary(end) {
        end -= 1;
    }
    &version[..end]
}

#[cfg(test)]
mod tests;
