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

/// Envelope version understood by the current job consumers.
const SUPPORTED_ENVELOPE_VERSION: &str = "v1";
/// Maximum number of UTF-8 bytes copied into logs and rejection diagnostics.
const MAX_VERSION_DIAGNOSTIC_BYTES: usize = 64;
const MALFORMED_PAYLOAD_MESSAGE: &str = "malformed job payload";

/// Decode a persisted job payload into its versioned envelope type `J`.
///
/// The `"v"` discriminant is checked before typed decoding so unsupported
/// versions remain distinct from malformed payloads. Unsupported-version
/// diagnostics escape control characters and contain at most 64 UTF-8 bytes.
/// The raw payload is never included because it can contain user data.
///
/// # Errors
///
/// Returns [`JobDispatchError::Rejected`] with an unsupported-version
/// diagnostic for a readable version other than `v1`. Missing or non-string
/// versions and otherwise malformed payloads return a fixed malformed-payload
/// diagnostic.
pub fn decode_job<J>(payload: &Value) -> Result<J, JobDispatchError>
where
    J: DeserializeOwned,
{
    let Some(version) = envelope_version(payload) else {
        return Err(malformed_payload_error());
    };
    if version != SUPPORTED_ENVELOPE_VERSION {
        return Err(JobDispatchError::rejected(format!(
            "unsupported job envelope version: {}",
            version_diagnostic(version)
        )));
    }

    J::deserialize(payload).map_err(|_error| malformed_payload_error())
}

/// Read the `"v"` discriminant as a string slice for diagnostics.
fn envelope_version(payload: &Value) -> Option<&str> {
    payload.get("v").and_then(Value::as_str)
}

fn malformed_payload_error() -> JobDispatchError {
    JobDispatchError::rejected(MALFORMED_PAYLOAD_MESSAGE)
}

fn version_diagnostic(version: &str) -> String {
    let mut diagnostic = String::new();
    for character in version.chars() {
        let fragment = escaped_version_character(character);
        if diagnostic.len() + fragment.len() > MAX_VERSION_DIAGNOSTIC_BYTES {
            break;
        }
        diagnostic.push_str(&fragment);
    }
    diagnostic
}

fn escaped_version_character(character: char) -> String {
    match character {
        '\n' => "\\n".to_owned(),
        '\r' => "\\r".to_owned(),
        '\t' => "\\t".to_owned(),
        '\u{1b}' => "\\x1b".to_owned(),
        '\u{2028}' => "\\u{2028}".to_owned(),
        '\u{2029}' => "\\u{2029}".to_owned(),
        '\\' => "\\\\".to_owned(),
        control if control.is_control() => format!("\\u{{{:x}}}", control as u32),
        printable => printable.to_string(),
    }
}

#[cfg(test)]
mod tests;
