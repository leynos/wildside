//! Fallback write for a terminal classification that could not be stored.
//!
//! The wrapper always returns an error from a failed delivery, so Apalis
//! acknowledges a terminal classification as `Failed`. A failed row is fetched
//! again while `attempts < max_attempts`, which means a terminal write that
//! fails would otherwise re-run a job the policy has already retired. Lowering
//! the cap to the terminating attempt before that acknowledgement closes the
//! window.
//!
//! One attempt is not enough on its own: this fallback runs only after another
//! write has already failed, so the usual cause is a database failing
//! transiently. The write is therefore retried a bounded number of times, and a
//! fallback that never lands raises an alarm rather than a warning. No handler
//! can protect a row whose writes the database refuses; the retry sequence and
//! its alarm are what keep that window as small as the delivery allows.

use std::time::Duration;

use tracing::{error, warn};

use super::{RetryFailureContext, RetryJobIdentity};
use crate::outbound::queue::TerminalCapClamp;

/// How many times the fallback write is attempted before it is abandoned.
const CLAMP_ATTEMPTS: usize = 3;

/// The waits before the second and third attempts.
///
/// The whole sequence costs less than a fifth of a second, which keeps the
/// failure path inside a delivery's budget while still riding out a blip.
const CLAMP_RETRY_BACKOFF: [Duration; CLAMP_ATTEMPTS - 1] =
    [Duration::from_millis(25), Duration::from_millis(150)];

/// Lower the row's attempt cap after a terminal write failed.
///
/// The write is retried with short waits while the database fails. A refused
/// guard is not retried: `Ok(false)` means the row is no longer this
/// delivery's, so another attempt would only repeat a decision another actor
/// already owns. An error that survives every attempt is logged as
/// `terminal_cap_clamp_exhausted`, which is the signal that a retired job may
/// be fetched again.
#[tracing::instrument(
    skip_all,
    fields(
        queue = context.queue_name,
        task_id = identity.task_id,
        worker_id = identity.worker_id,
        attempt = context.attempt.get(),
        decision = tracing::field::Empty,
        tries = tracing::field::Empty,
    )
)]
pub(super) async fn clamp_terminal_cap(
    context: &RetryFailureContext<'_>,
    identity: RetryJobIdentity<'_>,
) {
    let clamp = TerminalCapClamp {
        task_id: identity.task_id,
        worker_id: identity.worker_id,
        attempt: context.attempt,
    };
    // Each attempt after the first is preceded by its own wait, so the two
    // sequences are zipped rather than indexed.
    let waits = std::iter::once(None).chain(CLAMP_RETRY_BACKOFF.iter().map(Some));
    let mut last_error = None;
    for (tries, wait) in (1..=CLAMP_ATTEMPTS).zip(waits) {
        if let Some(wait) = wait {
            tokio::time::sleep(*wait).await;
        }
        match context.scheduler.clamp_terminal_cap(clamp).await {
            Ok(true) => {
                record_clamped(context, identity, tries);
                return;
            }
            Ok(false) => {
                record_refused(context, identity);
                return;
            }
            Err(error) => last_error = Some(error),
        }
    }
    if let Some(error) = last_error {
        record_exhausted(context, identity, &error);
    }
}

/// Record a fallback write that lowered the cap.
fn record_clamped(context: &RetryFailureContext<'_>, identity: RetryJobIdentity<'_>, tries: usize) {
    tracing::Span::current().record("decision", "terminal_cap_clamped");
    tracing::Span::current().record("tries", tries);
    warn!(
        queue = context.queue_name,
        task_id = identity.task_id,
        worker_id = identity.worker_id,
        attempt = context.attempt.get(),
        decision = "terminal_cap_clamped",
        tries,
        "job attempt cap lowered after a failed terminal update"
    );
}

/// Record a fallback write that no longer owns a running row.
fn record_refused(context: &RetryFailureContext<'_>, identity: RetryJobIdentity<'_>) {
    tracing::Span::current().record("decision", "terminal_cap_clamp_refused");
    warn!(
        queue = context.queue_name,
        task_id = identity.task_id,
        worker_id = identity.worker_id,
        attempt = context.attempt.get(),
        decision = "terminal_cap_clamp_refused",
        "job attempt cap not lowered: the row is no longer this delivery's"
    );
}

/// Record a fallback that failed every attempt.
///
/// The counter for a failed terminal write has already been incremented for
/// this delivery, so the alarm is a distinct decision label rather than a
/// second increment of the same metric.
fn record_exhausted(
    context: &RetryFailureContext<'_>,
    identity: RetryJobIdentity<'_>,
    error: &sqlx::Error,
) {
    tracing::Span::current().record("decision", "terminal_cap_clamp_exhausted");
    tracing::Span::current().record("tries", CLAMP_ATTEMPTS);
    error!(
        queue = context.queue_name,
        task_id = identity.task_id,
        worker_id = identity.worker_id,
        attempt = context.attempt.get(),
        decision = "terminal_cap_clamp_exhausted",
        tries = CLAMP_ATTEMPTS,
        error = %error,
        "job attempt cap not lowered after every attempt; a retired job may be fetched again"
    );
}
