//! Fallback write for a terminal classification that could not be stored.
//!
//! The wrapper always returns an error from a failed delivery, so Apalis
//! acknowledges a terminal classification as `Failed`. A failed row is fetched
//! again while `attempts < max_attempts`, which means a terminal write that
//! fails would otherwise re-run a job the policy has already retired. Lowering
//! the cap to the terminating attempt before that acknowledgement closes the
//! window.

use tracing::warn;

use super::{RetryFailureContext, RetryJobIdentity};
use crate::outbound::queue::TerminalCapClamp;

/// Lower the row's attempt cap after a terminal write failed.
///
/// Best effort by design: a refused guard means another actor owns the row, and
/// an error leaves the cap unchanged. Each outcome is logged with its decision
/// so an operator can tell which happened; nothing is retried here, because the
/// delivery is already unwinding.
#[tracing::instrument(
    skip_all,
    fields(
        queue = context.queue_name,
        task_id = identity.task_id,
        worker_id = identity.worker_id,
        attempt = context.attempt.get(),
        outcome = tracing::field::Empty,
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
    match context.scheduler.clamp_terminal_cap(clamp).await {
        Ok(true) => {
            tracing::Span::current().record("outcome", "terminal_cap_clamped");
            warn!(
                queue = context.queue_name,
                task_id = identity.task_id,
                worker_id = identity.worker_id,
                attempt = context.attempt.get(),
                decision = "terminal_cap_clamped",
                "job attempt cap lowered after a failed terminal update"
            );
        }
        Ok(false) => {
            tracing::Span::current().record("outcome", "terminal_cap_clamp_refused");
            warn!(
                queue = context.queue_name,
                task_id = identity.task_id,
                worker_id = identity.worker_id,
                attempt = context.attempt.get(),
                decision = "terminal_cap_clamp_refused",
                "job attempt cap not lowered: the row is no longer this delivery's"
            );
        }
        Err(error) => {
            tracing::Span::current().record("outcome", "terminal_cap_clamp_failed");
            warn!(
                queue = context.queue_name,
                task_id = identity.task_id,
                worker_id = identity.worker_id,
                attempt = context.attempt.get(),
                decision = "terminal_cap_clamp_failed",
                error = %error,
                "job attempt cap not lowered"
            );
        }
    }
}
