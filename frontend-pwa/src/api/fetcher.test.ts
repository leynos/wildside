/** @file Request and response contracts for the shared fetcher. */
import { afterEach, beforeEach, describe, expect, it, mock, spyOn } from 'bun:test';
import fc from 'fast-check';
import { z } from 'zod';

import { customFetch, customFetchOrval, customFetchParsed, customFetchParsedSafe } from './fetcher';

const userSchema = z.object({ id: z.string() });

let capturedRequest: RequestInit | undefined;
let response: Response;

function request(): RequestInit {
  if (!capturedRequest) throw new Error('No request was captured');
  return capturedRequest;
}

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

describe('customFetch', () => {
  it.each([
    ['object', { values: [1, 2] }, '{"values":[1,2]}', true],
    ['array', [true, 'x'], '[true,"x"]', true],
    ['number', 5, '5', true],
    ['boolean', true, 'true', true],
    ['JSON string', '{"ok":true}', '{"ok":true}', true],
    ['plain text string', 'plain text', 'plain text', false],
  ])('sends a %s body with matching headers', async (_label, body, expected, isJson) => {
    await customFetch('/api/example', { method: 'POST', body });

    expect(request().body).toBe(expected);
    expect(request().credentials).toBe('include');
    const headers = new Headers(request().headers);
    expect(headers.get('Accept')).toBe('application/json');
    expect(headers.has('Content-Type')).toBe(isJson);
    if (isJson) expect(headers.get('Content-Type')).toBe('application/json; charset=utf-8');
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

  it('includes plain-text error details in rejected responses', async () => {
    response = new Response('service unavailable', {
      status: 503,
      statusText: 'Service Unavailable',
      headers: { 'content-type': 'text/plain' },
    });

    await expect(customFetch('/api/example')).rejects.toThrow('service unavailable');
  });

  it('drops the detail when a JSON error body is malformed', async () => {
    response = new Response('not json', {
      status: 502,
      statusText: 'Bad Gateway',
      headers: { 'content-type': 'application/json' },
    });

    const rejection: Error = await customFetch('/api/example').then(
      () => {
        throw new Error('expected the request to be rejected');
      },
      (error: Error) => error,
    );
    expect(rejection).toBeInstanceOf(Error);
    expect(rejection.message).toContain('502 Bad Gateway');
    expect(rejection.message).not.toContain('not json');
  });

  it('rejects a successful response that is not valid JSON', async () => {
    response = new Response('not json', {
      status: 200,
      headers: { 'content-type': 'application/json' },
    });

    await expect(customFetch('/api/example')).rejects.toThrow(SyntaxError);
  });

  it('returns no value for an empty JSON body', async () => {
    response = new Response(undefined, {
      status: 200,
      headers: { 'content-type': 'application/json', 'content-length': '0' },
    });

    await expect(customFetch('/api/example')).resolves.toBeUndefined();
  });
});

type JsonValue = string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue };

// Keys that cannot collide with `Object.prototype` members, so generated
// objects survive `JSON.parse` as ordinary own properties.
const jsonKey = fc.stringMatching(/^[a-z][a-z0-9_]{0,7}$/);

/**
 * `fc.dictionary` builds null-prototype objects. Rebuilding through
 * `Object.fromEntries` yields a plain object, which is what the fetcher
 * serializes as a JSON structure.
 */
const jsonDictionary = (value: fc.Arbitrary<JsonValue>): fc.Arbitrary<JsonValue> =>
  fc
    .dictionary(jsonKey, value, { maxKeys: 4 })
    .map((entries) => Object.fromEntries(Object.entries(entries)));

/** Arbitrary recursive JSON document, including nested strings. */
const jsonValue = fc.letrec<{ value: JsonValue }>((tie) => ({
  value: fc.oneof(
    fc.integer(),
    fc.boolean(),
    fc.string(),
    fc.array(tie('value'), { maxLength: 4 }),
    jsonDictionary(tie('value')),
  ),
})).value;

/**
 * Structured and scalar bodies the fetcher must serialize. Top-level strings
 * and `null` take the explicit cases above instead.
 */
const jsonStructure = (): fc.Arbitrary<JsonValue> =>
  fc.oneof(
    fc.integer(),
    fc.boolean(),
    fc.array(jsonValue, { maxLength: 4 }),
    jsonDictionary(jsonValue),
  );

/** Mirror of the fetcher's own test for "this string is a JSON document". */
const parsesAsJson = (value: string): boolean => {
  try {
    JSON.parse(value);
    return true;
  } catch {
    return false;
  }
};

describe('customFetch body properties', () => {
  it('round-trips every JSON structure through its canonical serialization', async () => {
    await fc.assert(
      fc.asyncProperty(jsonStructure(), async (value) => {
        await customFetch('/api/example', { method: 'POST', body: value });

        // `JSON.stringify` is the canonical form, so byte equality here also
        // proves the parsed body preserves the original value.
        expect(String(request().body)).toBe(JSON.stringify(value));
        expect(new Headers(request().headers).get('Content-Type')).toBe(
          'application/json; charset=utf-8',
        );
      }),
    );
  });

  it('forwards string bodies verbatim and labels only JSON documents', async () => {
    await fc.assert(
      fc.asyncProperty(fc.string(), async (value) => {
        await customFetch('/api/example', { method: 'POST', body: value });

        expect(String(request().body)).toBe(value);
        const headers = new Headers(request().headers);
        expect(headers.has('Content-Type')).toBe(parsesAsJson(value));
      }),
    );
  });

  it('keeps a caller-supplied content type for any JSON body', async () => {
    await fc.assert(
      fc.asyncProperty(
        jsonStructure(),
        fc.stringMatching(/^[a-z]+\/[a-z0-9.+-]{1,20}$/),
        async (value, contentType) => {
          await customFetch('/api/example', {
            method: 'POST',
            body: value,
            headers: { 'Content-Type': contentType },
          });

          const headers = new Headers(request().headers);
          expect(headers.get('Content-Type')).toBe(contentType);
          expect(headers.get('Accept')).toBe('application/json');
          expect(String(request().body)).toBe(JSON.stringify(value));
        },
      ),
    );
  });
});

describe('customFetchOrval', () => {
  it('surfaces the response type while keeping transport validation', async () => {
    response = Response.json({ id: 'user-1' });

    await expect(customFetchOrval<{ id: string }>('/api/user')).resolves.toEqual({
      id: 'user-1',
    });

    response = new Response('missing', { status: 404 });

    await expect(customFetchOrval('/api/user')).rejects.toThrow('404');
  });
});

describe('customFetchParsed', () => {
  it('returns the payload validated by the caller schema', async () => {
    response = Response.json({ id: 'user-1', extra: true });

    await expect(customFetchParsed('/api/user', userSchema)).resolves.toEqual({ id: 'user-1' });
  });

  it('rejects when the payload does not match the schema', async () => {
    response = Response.json({ id: 7 });

    await expect(customFetchParsed('/api/user', userSchema)).rejects.toThrow();
  });
});

describe('customFetchParsedSafe', () => {
  it('reports a schema mismatch without rejecting', async () => {
    response = Response.json({ id: 7 });

    const result = await customFetchParsedSafe('/api/user', userSchema);

    expect(result.success).toBe(false);
  });

  it('still rejects non-2xx responses', async () => {
    response = Response.json({ error: 'nope' }, { status: 500 });

    await expect(customFetchParsedSafe('/api/user', userSchema)).rejects.toThrow('nope');
  });
});
