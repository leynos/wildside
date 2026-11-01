/** @file Property contracts for the bounded fetcher metrics consumer. */
import { describe, expect, it } from 'bun:test';
import fc from 'fast-check';

import type { FetchDiagnosticEvent, FetchFailureCategory } from './diagnostics';
import { createFetchMetrics, type OperationLatency } from './fetch-metrics';

const operations = ['/api/users', '/api/routes', '/api/v1/walk-sessions', '/:other'] as const;
const categories: readonly FetchFailureCategory[] = ['transport', 'http', 'json', 'schema'];
const methods = ['GET', 'POST', 'DELETE'] as const;

const requestEvent = fc.record<FetchDiagnosticEvent>({
  kind: fc.constant('request'),
  method: fc.constantFrom(...methods),
  operation: fc.constantFrom(...operations),
});

const responseEvent = fc.record<FetchDiagnosticEvent>({
  kind: fc.constant('response'),
  method: fc.constantFrom(...methods),
  operation: fc.constantFrom(...operations),
  durationMs: fc.option(fc.nat({ max: 5000 }), { nil: undefined }),
  statusClass: fc.constantFrom('2xx', '3xx'),
});

const failureEvent = fc.record<FetchDiagnosticEvent>({
  kind: fc.constant('failure'),
  method: fc.constantFrom(...methods),
  operation: fc.constantFrom(...operations),
  durationMs: fc.option(fc.nat({ max: 5000 }), { nil: undefined }),
  statusClass: fc.constantFrom('4xx', '5xx'),
  failureCategory: fc.constantFrom(...categories),
});

const anyEvent: fc.Arbitrary<FetchDiagnosticEvent> = fc.oneof(
  requestEvent,
  responseEvent,
  failureEvent,
);

interface ReferenceSnapshot {
  readonly requests: number;
  readonly responses: number;
  readonly failures: Record<FetchFailureCategory, number>;
  readonly latencyByOperation: Record<string, OperationLatency>;
  readonly recent: FetchDiagnosticEvent[];
}

/** Ceiling on operations the consumer may track individually. */
const MaxOperations = 32;
/** Ceiling on retained recent events. */
const MaxRecent = 50;

/** Mutable counts the reference fold accumulates. */
interface ReferenceCounts {
  requests: number;
  responses: number;
  failures: Record<FetchFailureCategory, number>;
}

const zeroFailures = (): Record<FetchFailureCategory, number> => ({
  transport: 0,
  http: 0,
  json: 0,
  schema: 0,
});

/** Apply the counting rules for one event, ignoring latency. */
const countEvent = (event: FetchDiagnosticEvent, counts: ReferenceCounts): void => {
  if (event.kind === 'request') {
    counts.requests += 1;
    return;
  }
  if (event.kind === 'response') {
    counts.responses += 1;
    return;
  }
  if (event.failureCategory) counts.failures[event.failureCategory] += 1;
};

/**
 * Duration an event contributes to its operation, if it contributes one.
 *
 * A schema failure repeats the response's duration, so it is not a second
 * observation of the same logical operation.
 */
const latencyOf = (event: FetchDiagnosticEvent): number | undefined =>
  event.kind !== 'request' && event.failureCategory !== 'schema' ? event.durationMs : undefined;

/** Fold one observation into an aggregate, starting from an empty one. */
const mergeLatency = (
  current: OperationLatency | undefined,
  durationMs: number,
): OperationLatency => {
  const previous = current ?? { count: 0, totalMs: 0, maxMs: 0 };
  return {
    count: previous.count + 1,
    totalMs: previous.totalMs + durationMs,
    maxMs: Math.max(previous.maxMs, durationMs),
  };
};

/**
 * Apply the latency rules for one completed event.
 *
 * New operations are refused once the bound is reached.
 */
const recordReferenceLatency = (
  event: FetchDiagnosticEvent,
  observed: Map<string, OperationLatency>,
): void => {
  const durationMs = latencyOf(event);
  if (durationMs === undefined) return;
  const current = observed.get(event.operation);
  if (!current && observed.size >= MaxOperations) return;
  observed.set(event.operation, mergeLatency(current, durationMs));
};

/**
 * Fold an event sequence into the snapshot the consumer promises.
 *
 * This deliberately re-derives the answer from the documented contract rather
 * than calling the consumer's own helpers, so a drift between contract and
 * implementation shows up as a failed property.
 */
const referenceSnapshot = (events: readonly FetchDiagnosticEvent[]): ReferenceSnapshot => {
  const counts: ReferenceCounts = { requests: 0, responses: 0, failures: zeroFailures() };
  const observed = new Map<string, OperationLatency>();

  for (const event of events) {
    countEvent(event, counts);
    recordReferenceLatency(event, observed);
  }

  return {
    ...counts,
    latencyByOperation: Object.fromEntries(observed),
    recent: events.slice(-MaxRecent),
  };
};

describe('createFetchMetrics properties', () => {
  it('matches an independent fold for every generated event sequence', () => {
    fc.assert(
      fc.property(fc.array(anyEvent, { maxLength: 80 }), (events) => {
        const metrics = createFetchMetrics();
        for (const event of events) metrics.sink(event);

        const snapshot = metrics.snapshot();
        const reference = referenceSnapshot(events);

        expect(snapshot.requests).toBe(reference.requests);
        expect(snapshot.responses).toBe(reference.responses);
        expect(snapshot.failures).toEqual(reference.failures);
        expect(snapshot.latencyByOperation).toEqual(reference.latencyByOperation);
        expect(snapshot.recent).toEqual(reference.recent);
      }),
      { numRuns: 200 },
    );
  });

  it('keeps latency aggregates internally consistent', () => {
    fc.assert(
      fc.property(fc.array(anyEvent, { maxLength: 120 }), (events) => {
        const metrics = createFetchMetrics();
        for (const event of events) metrics.sink(event);
        const { latencyByOperation } = metrics.snapshot();

        expect(Object.keys(latencyByOperation).length).toBeLessThanOrEqual(MaxOperations);
        for (const aggregate of Object.values(latencyByOperation)) {
          expect(aggregate.count).toBeGreaterThan(0);
          expect(aggregate.maxMs).toBeGreaterThanOrEqual(0);
          expect(aggregate.totalMs).toBeGreaterThanOrEqual(aggregate.maxMs);
        }
      }),
      { numRuns: 100 },
    );
  });

  it('keeps returning copies so callers cannot corrupt the aggregates', () => {
    fc.assert(
      fc.property(fc.array(anyEvent, { maxLength: 60 }), (events) => {
        const metrics = createFetchMetrics();
        for (const event of events) metrics.sink(event);

        const first = metrics.snapshot();
        Object.assign(first.latencyByOperation, {
          '/api/users': { count: 99, totalMs: 99, maxMs: 99 },
        });
        // SAFETY: `snapshot()` hands back a fresh array, so mutating it cannot
        // reach the consumer's own buffer. The public type is `readonly` to
        // stop ordinary callers mutating it, which is what this cast bypasses.
        (first.recent as FetchDiagnosticEvent[]).length = 0;

        const second = metrics.snapshot();
        expect(second).toEqual(referenceSnapshot(events));
      }),
      { numRuns: 100 },
    );
  });
});
