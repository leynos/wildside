/** @file Request and response contracts for the shared fetcher. */
import { afterEach, beforeEach, describe, expect, it, mock, spyOn } from 'bun:test';
import { z } from 'zod';

import { customFetch } from './fetcher';

let capturedRequest: RequestInit | undefined;
let response: Response;

function request(): RequestInit {
  if (!capturedRequest) throw new Error('No request was captured');
  return capturedRequest;
}

describe('customFetch', () => {
  beforeEach(() => {
    capturedRequest = undefined;
    response = Response.json({ ok: true });
    spyOn(globalThis, 'fetch').mockImplementation(
      Object.assign(
        (_input: Parameters<typeof fetch>[0], init?: Parameters<typeof fetch>[1]) => {
          capturedRequest = init;
          return Promise.resolve(response.clone());
        },
        { preconnect: globalThis.fetch.preconnect },
      ),
    );
  });

  afterEach(() => {
    mock.restore();
  });

  it.each([
    ['object', { values: [1, 2] }, '{"values":[1,2]}'],
    ['array', [true, 'x'], '[true,"x"]'],
    ['string', 'plain text', 'plain text'],
    ['JSON string', '{"ok":true}', '{"ok":true}'],
  ])('sends a %s body with JSON headers', async (_label, body, expected) => {
    await customFetch('/api/example', { method: 'POST', body });

    expect(request().body).toBe(expected);
    expect(request().credentials).toBe('include');
    const headers = new Headers(request().headers);
    expect(headers.get('Accept')).toBe('application/json');
    expect(headers.get('Content-Type')).toBe('application/json; charset=utf-8');
  });

  it.each([
    ['form', new FormData()],
    ['blob', new Blob(['data'])],
    ['URL parameters', new URLSearchParams({ q: 'test' })],
    ['array buffer', new ArrayBuffer(4)],
    ['buffer view', new Uint8Array([1, 2])],
    ['stream', new ReadableStream()],
  ])('preserves a %s body without adding a content type', async (_label, body) => {
    await customFetch('/api/example', { method: 'POST', body });

    expect(request().body).toBe(body);
    expect(new Headers(request().headers).has('Content-Type')).toBe(false);
  });

  it('preserves caller-supplied headers for a JSON body', async () => {
    await customFetch('/api/example', {
      method: 'POST',
      body: { value: 1 },
      headers: new Headers([
        ['Accept', 'application/vnd.example+json'],
        ['Content-Type', 'application/custom'],
      ]),
    });

    const headers = new Headers(request().headers);
    expect(headers.get('Accept')).toBe('application/vnd.example+json');
    expect(headers.get('Content-Type')).toBe('application/custom');
  });

  it('preserves absent and null request bodies', async () => {
    await customFetch('/api/example', { method: 'POST' });
    expect(request().body).toBeUndefined();
    expect(new Headers(request().headers).has('Content-Type')).toBe(false);

    const nullBody = z.null().parse(JSON.parse('null'));
    await customFetch('/api/example', { method: 'POST', body: nullBody });
    expect(request().body).toBeNull();
    expect(new Headers(request().headers).has('Content-Type')).toBe(false);
  });

  it('returns parsed JSON and no value for a 204 response', async () => {
    await expect(customFetch('/api/example')).resolves.toEqual({ ok: true });

    response = new Response(undefined, { status: 204 });
    await expect(customFetch('/api/example')).resolves.toBeUndefined();
  });

  it('includes JSON error details in rejected responses', async () => {
    response = Response.json({ error: 'invalid request' }, { status: 400 });

    await expect(customFetch('/api/example')).rejects.toThrow('invalid request');
  });
});
