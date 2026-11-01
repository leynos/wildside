/**
 * @file Client span lifecycle for the shared API fetcher.
 *
 * A span covers one logical request from the moment the fetcher starts building
 * it until the response has been read and decoded, so its duration includes
 * body consumption in the same way the diagnostics timing does. This is the
 * tracing half of the fetch boundary: diagnostics answer "how many, how long,
 * how often did it fail", while a span is the dated record of one crossing.
 *
 * A completed span carries only bounded values: the declared route template,
 * the known HTTP verb, the status class, and the bounded backend trace
 * identifier. It never carries a URL, query string, body, or credential. The
 * fetcher publishes spans through a sink the application installs, mirroring
 * `diagnostics.ts`; with no sink installed the fetcher behaves exactly as
 * before.
 */

import { elapsedSince, fetchClockNow } from './diagnostics';

/** How a span ended. A caller cancellation is not a fault. */
export type SpanStatus = 'ok' | 'error' | 'cancelled';

/** One completed client span at the network boundary. */
export interface ClientSpan {
  /** Declared route template, or `/unknown` for an unlisted path. */
  readonly name: string;
  /** Upper-case HTTP method, or `OTHER` for an unknown verb. */
  readonly method: string;
  /** Clock reading when the span started, in whole milliseconds. */
  readonly startedAt: number;
  /** Whole milliseconds from start to the end recorded here. */
  readonly durationMs: number;
  /** Whether the request completed, faulted, or was cancelled by its caller. */
  readonly status: SpanStatus;
  /** Status class such as `2xx`, when a response arrived. */
  readonly statusClass?: string;
  /** Bounded backend `trace-id`, when the response carried one. */
  readonly traceId?: string;
}

/** Receives completed client spans. */
export type SpanSink = (span: ClientSpan) => void;

let sink: SpanSink | undefined;

/** Install or clear the application-wide span sink. */
export const setSpanSink = (next?: SpanSink): void => {
  sink = next;
};

/**
 * Publish one completed span.
 *
 * A sink fault must never change the outcome of a request, so sink failures are
 * swallowed here rather than reaching the caller.
 */
export const emitSpan = (span: ClientSpan): void => {
  if (!sink) return;
  try {
    sink(span);
  } catch {
    // Tracing is best effort; request semantics take precedence.
  }
};

/** How a span in progress ended. */
export interface SpanOutcome {
  /** Whether the request completed, faulted, or was cancelled by its caller. */
  readonly status: SpanStatus;
  /** Status class such as `2xx`, when a response arrived. */
  readonly statusClass?: string;
  /** Bounded backend `trace-id`, when the response carried one. */
  readonly traceId?: string;
}

/** A span in progress, which publishes itself once when it ends. */
export interface ClientSpanHandle {
  /** Clock reading taken when the span started. */
  readonly startedAt: number;
  /** Publish the span; later calls are ignored so one request yields one span. */
  end(outcome: SpanOutcome): void;
}

/**
 * Start a span for one logical request.
 *
 * The caller ends it exactly once on every exit path. Ending twice is ignored
 * rather than duplicating the span, so a wrapper that reports both a fault and
 * a rethrow still contributes a single record.
 *
 * @example
 * const span = startClientSpan('GET', '/api/v1/users');
 * span.end({ status: 'ok', statusClass: '2xx' });
 */
export const startClientSpan = (method: string, name: string): ClientSpanHandle => {
  const startedAt = fetchClockNow();
  let isEnded = false;
  return {
    startedAt,
    end: (outcome) => {
      if (isEnded) return;
      isEnded = true;
      emitSpan({ name, method, startedAt, durationMs: elapsedSince(startedAt), ...outcome });
    },
  };
};
