/**
 * @file Bounded in-memory metrics for shared API fetcher diagnostics.
 *
 * The PWA has no telemetry vendor, so this consumer keeps the aggregates the
 * observability check asks for where the application can read them: request
 * counts, latency per operation, failure counts per bounded category, and a
 * bounded buffer of recent logical-operation records. Operation names come
 * from the bounded vocabulary in `diagnostics.ts`; trace identifiers are kept
 * only in the recent-record buffer and never become metric labels.
 */

import type {
  FetchDiagnosticEvent,
  FetchDiagnosticsSink,
  FetchFailureCategory,
} from './diagnostics';

/** Latency aggregate for one operation template. */
export interface OperationLatency {
  /** Number of completed observations for this operation. */
  readonly count: number;
  /** Sum of observed durations, in whole milliseconds. */
  readonly totalMs: number;
  /** Largest observed duration, in whole milliseconds. */
  readonly maxMs: number;
}

/** Point-in-time view of the fetcher metrics. */
export interface FetchMetricsSnapshot {
  /** Logical requests started. */
  readonly requests: number;
  /** Responses that completed without a transport, HTTP, or decoding fault. */
  readonly responses: number;
  /** Failures by bounded category. */
  readonly failures: Readonly<Record<FetchFailureCategory, number>>;
  /** Latency aggregates keyed by bounded operation template. */
  readonly latencyByOperation: Readonly<Record<string, OperationLatency>>;
  /** Most recent events, newest last, bounded by the buffer size. */
  readonly recent: readonly FetchDiagnosticEvent[];
}

/** A metrics consumer plus a snapshot accessor. */
export interface FetchMetrics {
  /** Sink that updates the aggregates. */
  readonly sink: FetchDiagnosticsSink;
  /** Current bounded view of the aggregates. */
  readonly snapshot: () => FetchMetricsSnapshot;
}

/** Operations tracked individually before new ones are ignored. */
const MaxOperations = 32;
/** Recent events retained for tracing. */
const MaxRecent = 50;

const emptyFailures = (): Record<FetchFailureCategory, number> => ({
  transport: 0,
  http: 0,
  json: 0,
  schema: 0,
});

const addLatency = (
  aggregate: OperationLatency | undefined,
  durationMs: number,
): OperationLatency => ({
  count: (aggregate?.count ?? 0) + 1,
  totalMs: (aggregate?.totalMs ?? 0) + durationMs,
  maxMs: Math.max(aggregate?.maxMs ?? 0, durationMs),
});

/**
 * Create a bounded metrics consumer for fetcher diagnostics.
 *
 * @example
 * const metrics = createFetchMetrics();
 * setFetchDiagnostics(metrics.sink);
 * metrics.snapshot().failures.http; // number of HTTP failures so far
 */
export const createFetchMetrics = (): FetchMetrics => {
  let requests = 0;
  let responses = 0;
  const failures = emptyFailures();
  const latency = new Map<string, OperationLatency>();
  const recent: FetchDiagnosticEvent[] = [];

  const remember = (event: FetchDiagnosticEvent): void => {
    recent.push(event);
    if (recent.length > MaxRecent) recent.shift();
  };

  const observeLatency = (event: FetchDiagnosticEvent): void => {
    if (event.durationMs === undefined) return;
    const current = latency.get(event.operation);
    if (!current && latency.size >= MaxOperations) return;
    latency.set(event.operation, addLatency(current, event.durationMs));
  };

  const sink: FetchDiagnosticsSink = (event) => {
    remember(event);
    if (event.kind === 'request') {
      requests += 1;
      return;
    }
    if (event.kind === 'response') {
      responses += 1;
      observeLatency(event);
      return;
    }
    if (event.failureCategory) failures[event.failureCategory] += 1;
    observeLatency(event);
  };

  const snapshot = (): FetchMetricsSnapshot => ({
    requests,
    responses,
    failures: { ...failures },
    latencyByOperation: Object.fromEntries(
      [...latency].map(([operation, aggregate]) => [operation, { ...aggregate }]),
    ),
    recent: [...recent],
  });

  return { sink, snapshot };
};

declare global {
  /** Snapshot accessor published for devtools; absent until the app installs it. */
  var __wildsideFetchMetrics: (() => FetchMetricsSnapshot) | undefined;
}

/**
 * Publish a snapshot accessor on the global object for devtools.
 *
 * The property is a function, not live state, so a developer console reads a
 * consistent view on demand. It exposes only bounded aggregates and recent
 * events, and the accessor holds no request data of its own.
 */
export const publishFetchMetrics = (metrics: FetchMetrics): void => {
  globalThis.__wildsideFetchMetrics = metrics.snapshot;
};
