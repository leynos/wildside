/** @file End-to-end fetcher behaviour against a real HTTP server. */

import { afterAll, beforeAll, beforeEach, describe, expect, it } from 'bun:test';
import { z } from 'zod';

import { customFetch, customFetchParsed, customFetchParsedSafe } from './fetcher';

interface ObservedRequest {
  method: string;
  body: string;
  contentType: string | null;
  accept: string | null;
}

let server: ReturnType<typeof globalThis.Bun.serve>;
let baseUrl: string;
let observed: ObservedRequest[] = [];

const userSchema = z.object({ id: z.string() });

/**
 * Serve the responses the fetcher contract has to handle. The server is a
 * real socket so the request and response bytes cross an HTTP boundary
 * instead of being intercepted by a fetch mock.
 */
function startServer(): ReturnType<typeof globalThis.Bun.serve> {
  return globalThis.Bun.serve({
    port: 0,
    async fetch(request) {
      const { pathname } = new URL(request.url);
      if (pathname === '/echo') {
        observed.push({
          method: request.method,
          body: await request.text(),
          contentType: request.headers.get('content-type'),
          accept: request.headers.get('accept'),
        });
        return Response.json({ ok: true });
      }
      if (pathname === '/user') return Response.json({ id: 'user-1' });
      if (pathname === '/no-content') return new Response(undefined, { status: 204 });
      if (pathname === '/failure') {
        return Response.json({ error: 'invalid request' }, { status: 400 });
      }
      if (pathname === '/text-failure') {
        return new Response('service unavailable', {
          status: 503,
          headers: { 'content-type': 'text/plain' },
        });
      }
      return new Response('missing', { status: 404 });
    },
  });
}

beforeAll(() => {
  server = startServer();
  baseUrl = `http://127.0.0.1:${server.port}`;
});

beforeEach(() => {
  observed = [];
});

afterAll(async () => {
  await server.stop(true);
});

describe('customFetch over HTTP', () => {
  it('posts a JSON body with default headers', async () => {
    await expect(
      customFetch(`${baseUrl}/echo`, { method: 'POST', body: { value: 1 } }),
    ).resolves.toEqual({ ok: true });

    expect(observed).toEqual([
      {
        method: 'POST',
        body: '{"value":1}',
        contentType: 'application/json; charset=utf-8',
        accept: 'application/json',
      },
    ]);
  });

  it('posts a plain-text body without a JSON content type', async () => {
    await expect(
      customFetch(`${baseUrl}/echo`, { method: 'POST', body: 'hello' }),
    ).resolves.toEqual({ ok: true });

    expect(observed).toHaveLength(1);
    expect(observed[0].body).toBe('hello');
    expect(observed[0].contentType).toBeNull();
  });

  it('returns no value for a 204 response', async () => {
    await expect(customFetch(`${baseUrl}/no-content`)).resolves.toBeUndefined();
  });

  it('reports JSON error details from a real response', async () => {
    await expect(customFetch(`${baseUrl}/failure`)).rejects.toThrow('invalid request');
  });

  it('reports plain-text error details from a real response', async () => {
    await expect(customFetch(`${baseUrl}/text-failure`)).rejects.toThrow('service unavailable');
  });
});

describe('parsed helpers over HTTP', () => {
  it('validates the payload with the caller schema', async () => {
    await expect(customFetchParsed(`${baseUrl}/user`, userSchema)).resolves.toEqual({
      id: 'user-1',
    });
  });

  it('surfaces schema failures through the safe helper', async () => {
    const result = await customFetchParsedSafe(`${baseUrl}/user`, z.object({ id: z.number() }));

    expect(result.success).toBe(false);
  });
});
