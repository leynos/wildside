/** @file Diagnostic events raised by the shared fetcher boundaries. */
import { afterEach, beforeEach, describe, expect, it, mock, spyOn } from 'bun:test';
import { z } from 'zod';

import { type FetchDiagnosticEvent, setFetchDiagnostics } from './diagnostics';
import { customFetch, customFetchParsed, customFetchParsedSafe } from './fetcher';

const userSchema = z.object({ id: z.string() });

let capturedRequest: RequestInit | undefined;
let response: Response;
let transportError: Error | undefined;
let events: FetchDiagnosticEvent[];

beforeEach(() => {
  capturedRequest = undefined;
  transportError = undefined;
  events = [];
  response = Response.json({ ok: true });
  setFetchDiagnostics((event) => events.push(event));
  spyOn(globalThis, 'fetch').mockImplementation(
    Object.assign(
      (_input: Parameters<typeof fetch>[0], init?: Parameters<typeof fetch>[1]) => {
        if (transportError) return Promise.reject(transportError);
        capturedRequest = init;
        return Promise.resolve(response.clone());
      },
      { preconnect: globalThis.fetch.preconnect },
    ),
  );
});

afterEach(() => {
  setFetchDiagnostics(undefined);
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

  it('classifies a malformed JSON response', async () => {
    response = new Response('not json', {
      status: 200,
      headers: { 'content-type': 'application/json' },
    });

    await expect(customFetch('/api/users')).rejects.toThrow(SyntaxError);

    expect(eventOf('failure').failureCategory).toBe('json');
  });
});

describe('schema diagnostics', () => {
  it('reports a schema failure from the rejecting helper', async () => {
    response = Response.json({ id: 7 });

    await expect(customFetchParsed('/api/users/42', userSchema)).rejects.toThrow();

    const failure = eventOf('failure');
    expect(failure.failureCategory).toBe('schema');
    expect(failure.operation).toBe('/api/users/:id');
  });

  it('reports a schema failure from the safe helper', async () => {
    response = Response.json({ id: 7 });

    const result = await customFetchParsedSafe('/api/users', userSchema);

    expect(result.success).toBe(false);
    expect(eventOf('failure').failureCategory).toBe('schema');
  });

  it('keeps a diagnostic sink fault from changing a request outcome', async () => {
    setFetchDiagnostics(() => {
      throw new Error('sink failed');
    });

    await expect(customFetch('/api/users')).resolves.toEqual({ ok: true });
    expect(capturedRequest).toBeDefined();
  });
});
