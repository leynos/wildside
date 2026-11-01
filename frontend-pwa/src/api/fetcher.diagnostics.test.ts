/** @file Diagnostic events raised by the shared fetcher boundaries. */
import { afterEach, beforeEach, describe, expect, it, mock, spyOn } from 'bun:test';
import { z } from 'zod';

import { type FetchDiagnosticEvent, setFetchClock, setFetchDiagnostics } from './diagnostics';
import { customFetch, customFetchParsed, customFetchParsedSafe } from './fetcher';

const userSchema = z.object({ id: z.string() });

let capturedRequest: RequestInit | undefined;
let response: Response;
let responseQueue: Response[];
let transportError: Error | undefined;
let events: FetchDiagnosticEvent[];

beforeEach(() => {
  capturedRequest = undefined;
  transportError = undefined;
  events = [];
  responseQueue = [];
  response = Response.json({ ok: true });
  setFetchDiagnostics((event) => events.push(event));
  spyOn(globalThis, 'fetch').mockImplementation(
    Object.assign(
      (_input: Parameters<typeof fetch>[0], init?: Parameters<typeof fetch>[1]) => {
        if (transportError) return Promise.reject(transportError);
        capturedRequest = init;
        const next = responseQueue.shift() ?? response;
        // Each test builds the response it expects, so the body may be consumed
        // once; returning the response itself lets a test stub `json` directly.
        return Promise.resolve(next);
      },
      { preconnect: globalThis.fetch.preconnect },
    ),
  );
});

afterEach(() => {
  setFetchDiagnostics(undefined);
  setFetchClock();
  mock.restore();
});

/** The event published for the given stage, failing when it is absent. */
function eventOf(kind: FetchDiagnosticEvent['kind']): FetchDiagnosticEvent {
  const event = events.find((candidate) => candidate.kind === kind);
  if (!event) throw new Error(`No ${kind} event was emitted`);
  return event;
}

describe('customFetch diagnostics', () => {
  it('reports a bounded request and response for a successful call', async () => {
    await customFetch('/api/users/42?expand=roles', { method: 'post' });

    expect(eventOf('request')).toEqual({
      kind: 'request',
      method: 'POST',
      operation: '/api/users/:id',
    });
    const responseEvent = eventOf('response');
    expect(responseEvent.method).toBe('POST');
    expect(responseEvent.operation).toBe('/api/users/:id');
    expect(responseEvent.statusClass).toBe('2xx');
    expect(responseEvent.durationMs).toBeGreaterThanOrEqual(0);
    expect(responseEvent.traceId).toBeUndefined();
  });

  it('captures the backend trace identifier as correlation context', async () => {
    response = Response.json({ ok: true }, { headers: { 'trace-id': 'trace-abc' } });

    await customFetch('/api/users');

    expect(eventOf('response').traceId).toBe('trace-abc');
  });

  it('classifies an HTTP failure without leaking the request URL', async () => {
    response = Response.json({ error: 'denied' }, { status: 403 });

    await expect(customFetch('/api/users/42?token=secret')).rejects.toThrow('403');

    const failure = eventOf('failure');
    expect(failure.failureCategory).toBe('http');
    expect(failure.statusClass).toBe('4xx');
    expect(failure.operation).toBe('/api/users/:id');
    expect(failure.operation).not.toContain('token');
  });

  it('classifies a transport failure and rethrows the original error', async () => {
    transportError = new Error('network down');

    await expect(customFetch('/api/users')).rejects.toThrow('network down');

    expect(eventOf('failure').failureCategory).toBe('transport');
  });

  it('does not report a caller-initiated abort as a transport failure', async () => {
    transportError = new DOMException('The operation was aborted.', 'AbortError');
    const controller = new AbortController();

    await expect(customFetch('/api/users', { signal: controller.signal })).rejects.toThrow(
      'aborted',
    );

    expect(events.filter((event) => event.kind === 'failure')).toHaveLength(0);
  });

  it('still reports a timeout as a transport failure', async () => {
    transportError = new DOMException('The operation timed out.', 'TimeoutError');

    await expect(customFetch('/api/users')).rejects.toThrow('timed out');

    expect(eventOf('failure').failureCategory).toBe('transport');
  });

  it('does not report a caller abort during a body read as a JSON failure', async () => {
    const controller = new AbortController();
    // The body stays pending until the caller aborts, so the rejection carries
    // the caller's own signal reason rather than a stream that failed on its
    // own.
    const pendingBody = new ReadableStream<Uint8Array>({
      start(stream) {
        controller.signal.addEventListener(
          'abort',
          () => {
            stream.error(controller.signal.reason);
          },
          { once: true },
        );
      },
    });
    response = new Response(pendingBody, {
      headers: { 'content-type': 'application/json' },
    });

    const request = customFetch('/api/users', { signal: controller.signal });
    controller.abort();

    await expect(request).rejects.toThrow();

    expect(events.filter((event) => event.kind === 'failure')).toHaveLength(0);
  });

  it('treats a rejection equal to the caller signal reason as a cancellation', async () => {
    const controller = new AbortController();
    const reason = new Error('cancelled by caller');
    transportError = reason;

    const request = customFetch('/api/users', { signal: controller.signal });
    controller.abort(reason);

    await expect(request).rejects.toBe(reason);

    expect(events.filter((event) => event.kind === 'failure')).toHaveLength(0);
  });

  it('classifies a malformed JSON response', async () => {
    response = new Response('not json', {
      status: 200,
      headers: { 'content-type': 'application/json' },
    });

    await expect(customFetch('/api/users')).rejects.toThrow(SyntaxError);

    expect(eventOf('failure').failureCategory).toBe('json');
  });

  it('includes body consumption in the reported duration', async () => {
    // The injected clock keeps the assertion exact: the request starts at 0
    // and reading the body advances the clock, so the response event must
    // report the full 40 ms rather than the sub-millisecond transport time.
    let nowMs = 0;
    setFetchClock(() => nowMs);
    const slow = Response.json({ ok: true });
    const readJson = slow.json.bind(slow);
    slow.json = async () => {
      nowMs = 40;
      return readJson();
    };
    response = slow;

    await customFetch('/api/users');

    expect(eventOf('response').durationMs).toBe(40);
  });

  it('correlates concurrent requests to the same route by trace id', async () => {
    responseQueue = [
      Response.json({ id: 'a' }, { headers: { 'trace-id': 'trace-1' } }),
      Response.json({ id: 7 }, { headers: { 'trace-id': 'trace-2' } }),
    ];

    const [accepted, rejected] = await Promise.all([
      customFetchParsed('/api/users/42', userSchema),
      customFetchParsedSafe('/api/users/42', userSchema),
    ]);

    expect(accepted).toEqual({ id: 'a' });
    expect(rejected.success).toBe(false);
    const failures = events.filter((event) => event.kind === 'failure');
    expect(failures).toHaveLength(1);
    expect(failures[0].traceId).toBe('trace-2');
    expect(failures[0].operation).toBe('/api/users/:id');
  });
});

describe('schema diagnostics', () => {
  it('reports a schema failure from the rejecting helper', async () => {
    response = Response.json({ id: 7 }, { headers: { 'trace-id': 'trace-schema' } });

    await expect(customFetchParsed('/api/users/42', userSchema)).rejects.toThrow();

    const failure = eventOf('failure');
    expect(failure.failureCategory).toBe('schema');
    expect(failure.operation).toBe('/api/users/:id');
    expect(failure.statusClass).toBe('2xx');
    expect(failure.traceId).toBe('trace-schema');
    const transportDuration = eventOf('response').durationMs ?? 0;
    expect(failure.durationMs ?? 0).toBeGreaterThanOrEqual(transportDuration);
  });

  it('reports a schema failure from the safe helper', async () => {
    response = Response.json({ id: 7 }, { headers: { 'trace-id': 'trace-safe' } });

    const result = await customFetchParsedSafe('/api/users', userSchema);

    expect(result.success).toBe(false);
    const failure = eventOf('failure');
    expect(failure.failureCategory).toBe('schema');
    expect(failure.traceId).toBe('trace-safe');
  });

  it('keeps a diagnostic sink fault from changing a request outcome', async () => {
    setFetchDiagnostics(() => {
      throw new Error('sink failed');
    });

    await expect(customFetch('/api/users')).resolves.toEqual({ ok: true });
    expect(capturedRequest).toBeDefined();
  });
});
