/**
 * @file Fetch helper that augments requests with common defaults.
 */

import type { z } from 'zod';

// --- Small helpers to keep complexity low and intent clear ---

const apiBase = (): string => import.meta.env.VITE_API_BASE ?? 'http://localhost:8080';

/** Predicate: does value represent a native non-JSON body type? */
const isNativeBodyType = (value: unknown): boolean => {
  return (
    (typeof FormData !== 'undefined' && value instanceof FormData) ||
    (typeof Blob !== 'undefined' && value instanceof Blob) ||
    (typeof URLSearchParams !== 'undefined' && value instanceof URLSearchParams) ||
    (typeof ArrayBuffer !== 'undefined' &&
      (value instanceof ArrayBuffer || ArrayBuffer.isView(value as ArrayBufferView)))
  );
};

/** Predicate: treat POJOs and arrays as JSON serialisable structures. */
const isPlainObjectLike = (value: unknown): boolean =>
  typeof value === 'object' &&
  value !== null &&
  (Array.isArray(value) || Object.getPrototypeOf(value) === Object.prototype);

/** Heuristic for strings that look like JSON payloads. */
const looksLikeJsonString = (text: string): boolean => {
  const trimmed = text.trim();
  return (
    (trimmed.startsWith('{') && trimmed.endsWith('}')) ||
    (trimmed.startsWith('[') && trimmed.endsWith(']'))
  );
};

/**
 * Normalise the request body for JSON APIs.
 * - Auto-JSON-stringify plain objects to reduce caller friction and header mistakes.
 * - Preserve native body types (FormData, Blob, URLSearchParams, ArrayBuffer/View).
 *
 * Returns the possibly transformed body and whether we produced JSON.
 */
const normaliseBody = (body: unknown): { body: BodyInit | null | undefined; isJson: boolean } => {
  // Be explicit to satisfy strict‑equality lint rules and avoid coercion.
  if (body === null || body === undefined) {
    return { body: body as null | undefined, isJson: false };
  }

  if (isNativeBodyType(body)) return { body: body as BodyInit, isJson: false };

  if (isPlainObjectLike(body)) return { body: JSON.stringify(body), isJson: true };

  if (typeof body === 'string') return { body, isJson: looksLikeJsonString(body) };

  return { body: body as BodyInit, isJson: false };
};

/**
 * Build default headers and attach JSON content-type when appropriate.
 * Keeps header logic centralised so callers do not need to manage it.
 */
const defaultHeaders = (init: RequestInit | undefined, bodyInfo: { isJson: boolean }): Headers => {
  const headers = new Headers(init?.headers as HeadersInit | undefined);
  if (!headers.has('Accept')) headers.set('Accept', 'application/json');

  // Only set Content-Type if not specified by caller and we know it's JSON.
  if (bodyInfo.isJson && !headers.has('Content-Type')) {
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

  // Normalise body upfront so header logic sees the final representation.
  const { body, isJson } = normaliseBody(init?.body as unknown);
  const headers = defaultHeaders(init, { isJson });

  const res = await fetch(url, { credentials: 'include', ...init, body, headers });

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
