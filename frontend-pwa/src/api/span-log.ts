/**
 * @file Bounded in-memory span log for the client fetch boundary.
 *
 * The PWA has no telemetry destination yet, so the consumer keeps completed
 * client spans where the application and devtools can read them. The buffer is
 * bounded, and a snapshot hands back a copy, so a reader can never grow the log
 * or edit a retained span.
 */

import type { ClientSpan, SpanSink } from './tracing';

/** Point-in-time view of the span log, oldest first. */
export interface SpanLogSnapshot {
  /** Retained spans, oldest first. */
  readonly spans: readonly ClientSpan[];
}

/** A span consumer plus a snapshot accessor. */
export interface SpanLog {
  /** Sink that retains completed spans. */
  readonly sink: SpanSink;
  /** Current bounded view of the retained spans. */
  readonly snapshot: () => SpanLogSnapshot;
}

/** Spans retained before the oldest is discarded. */
const MaxSpans = 100;

/**
 * Create a bounded span log.
 *
 * @example
 * const spans = createSpanLog();
 * setSpanSink(spans.sink);
 * spans.snapshot().spans; // retained client spans, oldest first
 */
export const createSpanLog = (limit = MaxSpans): SpanLog => {
  const retained: ClientSpan[] = [];

  const sink: SpanSink = (span) => {
    retained.push(span);
    if (retained.length > limit) retained.shift();
  };

  const snapshot = (): SpanLogSnapshot => ({ spans: [...retained] });

  return { sink, snapshot };
};

declare global {
  /** Span-log accessor published for devtools; absent until the app installs it. */
  var __wildsideClientSpans: (() => SpanLogSnapshot) | undefined;
}

/**
 * Publish a span-log accessor on the global object for devtools.
 *
 * The property is a function, not live state, so a developer console reads a
 * consistent view on demand. It exposes only bounded spans.
 */
export const publishSpanLog = (log: SpanLog): void => {
  globalThis.__wildsideClientSpans = log.snapshot;
};
