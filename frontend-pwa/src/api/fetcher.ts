/**
 * @file Fetch helper that augments requests with common defaults.
 */

import type { z } from 'zod';

export const customFetch = async <T>(input: string, init?: RequestInit): Promise<T> => {
  const base = import.meta.env.VITE_API_BASE ?? 'http://localhost:8080';
  const url = new URL(input, base);
  const headers = new Headers(init?.headers as HeadersInit | undefined);
  if (!headers.has('Accept')) headers.set('Accept', 'application/json');
  if (init?.body != null && !headers.has('Content-Type')) {
    const b = init.body as unknown;
    const isMultipart = typeof FormData !== 'undefined' && b instanceof FormData;
    const isBlob = typeof Blob !== 'undefined' && b instanceof Blob;
    const isUrlEncoded = typeof URLSearchParams !== 'undefined' && b instanceof URLSearchParams;
    const isBinary =
      typeof ArrayBuffer !== 'undefined' &&
      (b instanceof ArrayBuffer || ArrayBuffer.isView(b as ArrayBufferView));
    if (!isMultipart && !isBlob && !isUrlEncoded && !isBinary && typeof b === 'string') {
      headers.set('Content-Type', 'application/json');
    }
  }

  const res = await fetch(url, {
    credentials: 'include',
    ...init,
    headers,
  });

  if (!res.ok) {
    let detail: unknown;
    const ct = res.headers.get('content-type') ?? '';
    try {
      detail = ct.includes('json') ? await res.json() : await res.text();
    } catch {
      /* ignore parse errors */
    }
    throw new Error(
      `${res.status} ${res.statusText} ${res.url}${detail ? `: ${JSON.stringify(detail)}` : ''}`,
      { cause: detail },
    );
  }

  if (res.status === 204) return undefined as unknown as T;
  const ct = res.headers.get('content-type') ?? '';
  const len = res.headers.get('content-length');
  if (!ct.includes('json') || len === '0') {
    return undefined as unknown as T;
  }
  return res.json() as Promise<T>;
};

/**
 * Fetch JSON and validate it against a provided Zod schema.
 */
export const customFetchParsed = async <T>(
  input: string,
  schema: z.ZodType<T>,
  init?: RequestInit,
): Promise<T> => {
  const data = await customFetch<unknown>(input, init);
  return schema.parse(data);
};
