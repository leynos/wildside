/** @file Contracts for the bounded fetcher metrics consumer. */
import { afterEach, describe, expect, it } from 'bun:test';

import type { FetchDiagnosticEvent, FetchFailureCategory } from './diagnostics';
import {
  createFetchMetrics,
  type FetchMetricsSnapshot,
  publishFetchMetrics,
} from './fetch-metrics';

const requestEvent = (operation: string): FetchDiagnosticEvent => ({
  kind: 'request',
  method: 'GET',
  operation,
});

const responseEvent = (operation: string, durationMs: number): FetchDiagnosticEvent => ({
  kind: 'response',
  method: 'GET',
  operation,
  durationMs,
  statusClass: '2xx',
});

const failureEvent = (
  operation: string,
  failureCategory: FetchFailureCategory,
  durationMs: number,
): FetchDiagnosticEvent => ({
  kind: 'failure',
  method: 'GET',
  operation,
  durationMs,
  statusClass: '5xx',
  failureCategory,
});

describe('createFetchMetrics', () => {
  it('counts requests and successful responses separately', () => {
    const metrics = createFetchMetrics();

    metrics.sink(requestEvent('/api/users'));
    metrics.sink(responseEvent('/api/users', 12));

    const snapshot = metrics.snapshot();
    expect(snapshot.requests).toBe(1);
    expect(snapshot.responses).toBe(1);
    expect(snapshot.failures).toEqual({ transport: 0, http: 0, json: 0, schema: 0 });
  });

  it('counts failures by bounded category', () => {
    const metrics = createFetchMetrics();

    metrics.sink(failureEvent('/api/users', 'http', 20));
    metrics.sink(failureEvent('/api/users', 'http', 30));
    metrics.sink(failureEvent('/api/routes', 'schema', 40));

    const snapshot = metrics.snapshot();
    expect(snapshot.failures).toEqual({ transport: 0, http: 2, json: 0, schema: 1 });
    expect(snapshot.responses).toBe(0);
  });

  it('aggregates latency per operation template', () => {
    const metrics = createFetchMetrics();

    metrics.sink(responseEvent('/api/users', 10));
    metrics.sink(responseEvent('/api/users', 30));
    metrics.sink(failureEvent('/api/routes', 'json', 5));

    const snapshot = metrics.snapshot();
    expect(snapshot.latencyByOperation['/api/users']).toEqual({
      count: 2,
      totalMs: 40,
      maxMs: 30,
    });
    expect(snapshot.latencyByOperation['/api/routes']).toEqual({
      count: 1,
      totalMs: 5,
      maxMs: 5,
    });
  });

  it('records one latency observation when a response is followed by a schema failure', () => {
    const metrics = createFetchMetrics();

    metrics.sink(responseEvent('/api/users', 12));
    metrics.sink(failureEvent('/api/users', 'schema', 18));

    expect(metrics.snapshot().latencyByOperation['/api/users']).toEqual({
      count: 1,
      totalMs: 12,
      maxMs: 12,
    });
    expect(metrics.snapshot().failures.schema).toBe(1);
  });

  it('ignores events without a duration', () => {
    const metrics = createFetchMetrics();

    metrics.sink(requestEvent('/api/users'));

    expect(metrics.snapshot().latencyByOperation).toEqual({});
  });

  it('bounds the number of tracked operations', () => {
    const metrics = createFetchMetrics();

    for (let index = 0; index < 40; index += 1) {
      metrics.sink(responseEvent(`/api/op-${index}`, index));
    }

    expect(Object.keys(metrics.snapshot().latencyByOperation)).toHaveLength(32);
  });

  it('retains a bounded recent buffer with trace ids, newest last', () => {
    const metrics = createFetchMetrics();

    for (let index = 0; index < 60; index += 1) {
      metrics.sink({ ...responseEvent('/api/users', index), traceId: `trace-${index}` });
    }

    const { recent } = metrics.snapshot();
    expect(recent).toHaveLength(50);
    expect(recent[0].traceId).toBe('trace-10');
    expect(recent[49].traceId).toBe('trace-59');
  });

  it('returns a copy so callers cannot mutate the aggregates', () => {
    const metrics = createFetchMetrics();
    metrics.sink(responseEvent('/api/users', 10));

    const snapshot = metrics.snapshot();
    Object.assign(snapshot.latencyByOperation, {
      '/api/users': { count: 99, totalMs: 99, maxMs: 99 },
    });

    expect(metrics.snapshot().latencyByOperation['/api/users']).toEqual({
      count: 1,
      totalMs: 10,
      maxMs: 10,
    });
  });
});

describe('publishFetchMetrics', () => {
  afterEach(() => {
    globalThis.__wildsideFetchMetrics = undefined;
  });

  it('exposes a snapshot accessor without using trace ids as labels', () => {
    const metrics = createFetchMetrics();
    metrics.sink({ ...responseEvent('/api/users', 10), traceId: 'trace-abc' });

    publishFetchMetrics(metrics);

    const published = globalThis.__wildsideFetchMetrics;
    if (!published) throw new Error('the snapshot accessor was not published');
    const snapshot: FetchMetricsSnapshot = published();
    expect(snapshot).toEqual(metrics.snapshot());
    expect(Object.keys(snapshot.latencyByOperation)).toEqual(['/api/users']);
    expect(snapshot.recent[0].traceId).toBe('trace-abc');
  });
});
