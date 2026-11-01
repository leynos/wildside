/** @file Client spans recorded at the shared fetcher boundary. */
import { afterEach, beforeEach, describe, expect, it, mock, spyOn } from 'bun:test';

import { setFetchClock } from './diagnostics';
import { customFetch } from './fetcher';
import { type ClientSpan, setSpanSink } from './tracing';

let response: Response;
let transportError: Error | undefined;
let spans: ClientSpan[];

beforeEach(() => {
  transportError = undefined;
  spans = [];
  response = Response.json({ ok: true });
  setSpanSink((span) => spans.push(span));
  spyOn(globalThis, 'fetch').mockImplementation(
    Object.assign(
      () => {
        if (transportError) return Promise.reject(transportError);
        return Promise.resolve(response);
      },
      { preconnect: globalThis.fetch.preconnect },
    ),
  );
});

afterEach(() => {
  setSpanSink(undefined);
  setFetchClock();
  mock.restore();
});

/** The single span the boundary must have recorded, failing when it did not. */
function onlySpan(): ClientSpan {
  expect(spans).toHaveLength(1);
  return spans[0];
}

describe('fetcher client spans', () => {
  it('records one ok span for a successful request', async () => {
    let nowMs = 0;
    setFetchClock(() => nowMs);
    response = Response.json({ ok: true }, { headers: { 'trace-id': 'trace-abc' } });

    const request = customFetch('/api/v1/users/42?expand=roles', { method: 'post' });
    nowMs = 9;
    await request;

    expect(onlySpan()).toEqual({
      name: '/api/v1/users/:id',
      method: 'POST',
      startedAt: 0,
      durationMs: 9,
      status: 'ok',
      statusClass: '2xx',
      traceId: 'trace-abc',
    });
  });

  it('records an error span carrying the response status class', async () => {
    response = Response.json({ error: 'denied' }, { status: 503 });

    await expect(customFetch('/api/v1/routes/42/notes')).rejects.toThrow('503');

    expect(onlySpan()).toMatchObject({
      name: '/api/v1/routes/:id/notes',
      method: 'GET',
      status: 'error',
      statusClass: '5xx',
    });
  });

  it('records an error span without a status class when no response arrived', async () => {
    transportError = new Error('network down');

    await expect(customFetch('/api/v1/users')).rejects.toThrow('network down');

    const span = onlySpan();
    expect(span.status).toBe('error');
    expect(span.statusClass).toBeUndefined();
  });

  it('records a caller abort as cancelled rather than as an error', async () => {
    transportError = new DOMException('The operation was aborted.', 'AbortError');
    const controller = new AbortController();

    await expect(customFetch('/api/v1/users', { signal: controller.signal })).rejects.toThrow(
      'aborted',
    );

    expect(onlySpan()).toMatchObject({ status: 'cancelled' });
  });

  it('records an error span when the payload is not JSON', async () => {
    response = new Response('not json', {
      status: 200,
      headers: { 'content-type': 'application/json' },
    });

    await expect(customFetch('/api/v1/users')).rejects.toThrow(SyntaxError);

    expect(onlySpan()).toMatchObject({ status: 'error', statusClass: '2xx' });
  });

  it('bounds the span name and method the caller can choose', async () => {
    await customFetch('/internal/widgets', { method: 'break-all' });

    expect(onlySpan()).toMatchObject({ name: '/unknown', method: 'OTHER' });
  });
});
