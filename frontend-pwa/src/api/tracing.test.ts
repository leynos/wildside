/** @file Span lifecycle contracts for the shared fetch boundary. */
import { afterEach, describe, expect, it } from 'bun:test';

import { setFetchClock } from './diagnostics';
import { type ClientSpan, emitSpan, setSpanSink, startClientSpan } from './tracing';

const sampleSpan: ClientSpan = {
  name: '/api/v1/users',
  method: 'GET',
  startedAt: 0,
  durationMs: 1,
  status: 'ok',
};

afterEach(() => {
  setSpanSink(undefined);
  setFetchClock();
});

describe('startClientSpan', () => {
  it('publishes one bounded span when it ends', () => {
    let nowMs = 0;
    setFetchClock(() => nowMs);
    const spans: ClientSpan[] = [];
    setSpanSink((span) => spans.push(span));

    const span = startClientSpan('GET', '/api/v1/users');
    nowMs = 12;
    span.end({ status: 'ok', statusClass: '2xx', traceId: 'trace-abc' });

    expect(spans).toEqual([
      {
        name: '/api/v1/users',
        method: 'GET',
        startedAt: 0,
        durationMs: 12,
        status: 'ok',
        statusClass: '2xx',
        traceId: 'trace-abc',
      },
    ]);
  });

  it('publishes nothing until the span ends', () => {
    const spans: ClientSpan[] = [];
    setSpanSink((span) => spans.push(span));

    startClientSpan('POST', '/api/v1/users');

    expect(spans).toHaveLength(0);
  });

  it('ignores a second end so one request yields one span', () => {
    const spans: ClientSpan[] = [];
    setSpanSink((span) => spans.push(span));
    const span = startClientSpan('GET', '/api/v1/users');

    span.end({ status: 'error', statusClass: '5xx' });
    span.end({ status: 'ok', statusClass: '2xx' });

    expect(spans).toHaveLength(1);
    expect(spans[0].status).toBe('error');
  });

  it('records an error span without a status class when no response arrived', () => {
    const spans: ClientSpan[] = [];
    setSpanSink((span) => spans.push(span));

    startClientSpan('GET', '/unknown').end({ status: 'error' });

    expect(spans[0]).toMatchObject({ name: '/unknown', status: 'error' });
    expect(spans[0].statusClass).toBeUndefined();
  });

  it('keeps a request outcome when the span sink throws', () => {
    setSpanSink(() => {
      throw new Error('sink failed');
    });
    const span = startClientSpan('GET', '/api/v1/users');

    expect(() => span.end({ status: 'ok' })).not.toThrow();
  });

  it('emits nothing while no sink is installed', () => {
    expect(() => emitSpan(sampleSpan)).not.toThrow();
  });
});
