/**
 * @file Fetch helper that augments requests with common defaults.
 */

import type { z } from 'zod';

// --- Small helpers to keep complexity low and intent clear ---

const apiBase = (): string => import.meta.env.VITE_API_BASE ?? 'http://localhost:8080';

const shouldAddJsonContentType = (body: unknown, headers: Headers): boolean => {
  if (headers.has('Content-Type')) return false;
  const isMultipart = typeof FormData !== 'undefined' && body instanceof FormData;
  const isBlob = typeof Blob !== 'undefined' && body instanceof Blob;
  const isUrlEncoded = typeof URLSearchParams !== 'undefined' && body instanceof URLSearchParams;
  const isBinary =
    typeof ArrayBuffer !== 'undefined' &&
    (body instanceof ArrayBuffer || ArrayBuffer.isView(body as ArrayBufferView));
  return !isMultipart && !isBlob && !isUrlEncoded && !isBinary && typeof body === 'string';
};

const ensureDefaultHeaders = (init?: RequestInit): Headers => {
  const headers = new Headers(init?.headers as HeadersInit | undefined);
  if (!headers.has('Accept')) headers.set('Accept', 'application/json');
  const rawBody = init?.body as unknown;
  if (rawBody !== null && rawBody !== undefined && shouldAddJsonContentType(rawBody, headers)) {
    headers.set('Content-Type', 'application/json');
  }
  return headers;
};

const parseErrorDetail = async (res: Response): Promise<unknown> => {
  const ct = res.headers.get('content-type') ?? '';
  try {
    return ct.includes('json') ? await res.json() : await res.text();
  } catch {
    return undefined;
  }
};

const shouldParseJson = (res: Response): boolean => {
  if (res.status === 204) return false;
  const ct = res.headers.get('content-type') ?? '';
  const len = res.headers.get('content-length');
  return ct.includes('json') && len !== '0';
};

export const customFetch = async <T>(input: string, init?: RequestInit): Promise<T> => {
  const url = new URL(input, apiBase());
  const headers = ensureDefaultHeaders(init);

  const res = await fetch(url, { credentials: 'include', ...init, headers });

  if (!res.ok) {
    const detail = await parseErrorDetail(res);
    const suffix = detail ? `: ${JSON.stringify(detail)}` : '';
    throw new Error(`${res.status} ${res.statusText} ${res.url}${suffix}`, { cause: detail });
  }

  if (!shouldParseJson(res)) return undefined as unknown as T;
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
