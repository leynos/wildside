/**
 * @file Fetch helper that augments requests with common defaults.
 */

import type { z } from 'zod';

// --- Small helpers to keep complexity low and intent clear ---

const apiBase = (): string => import.meta.env.VITE_API_BASE ?? 'http://localhost:8080';

// Content-type predicates kept small and explicit to reduce complexity
const isFormData = (body: unknown): boolean =>
  typeof FormData !== 'undefined' && body instanceof FormData;
const isBlob = (body: unknown): boolean => typeof Blob !== 'undefined' && body instanceof Blob;
const isUrlEncoded = (body: unknown): boolean =>
  typeof URLSearchParams !== 'undefined' && body instanceof URLSearchParams;
const isBinary = (body: unknown): boolean =>
  typeof ArrayBuffer !== 'undefined' &&
  (body instanceof ArrayBuffer || ArrayBuffer.isView(body as ArrayBufferView));
const isStringBody = (body: unknown): boolean => typeof body === 'string';

// Decide whether we should set JSON Content‑Type for this request.
const shouldSetJsonContentType = (
  headers: Headers,
  bodyInfo: { isJson: boolean },
  rawBody: unknown,
): boolean => {
  if (headers.has('Content-Type')) return false;
  if (bodyInfo.isJson) return true;
  return getContentTypeForBody(rawBody) !== null;
};

// (former shouldAddJsonContentType) merged into defaultHeaders to keep logic in one place

/** Predicate: does value represent a native non-JSON body type? */
const isNativeBodyType = (value: unknown): boolean => {
  return isFormData(value) || isBlob(value) || isUrlEncoded(value) || isBinary(value);
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

  // Only set Content-Type if not specified by caller and it's clearly JSON.
  // We consider both auto-JSON (plain objects) and string bodies for legacy compatibility.
  const rawBody = init?.body as unknown;
  if (shouldSetJsonContentType(headers, bodyInfo, rawBody)) {
    headers.set('Content-Type', 'application/json; charset=utf-8');
  }
  return headers;
};

const parseErrorDetail = async (res: Response): Promise<unknown> => {
  const ct = (res.headers.get('content-type') ?? '').trim().toLowerCase();
  try {
    return ct.includes('json') ? await res.json() : await res.text();
  } catch {
    return undefined;
  }
};

const shouldParseJson = (res: Response): boolean => {
  if (res.status === 204 || res.status === 205) return false;
  const ct = (res.headers.get('content-type') ?? '').trim().toLowerCase();
  const len = res.headers.get('content-length');
  const hasBody = len == null || len !== '0';
  return ct.includes('json') && hasBody;
};

/**
 * Decide the appropriate Content-Type for a given raw request body.
 *
 * Returns 'application/json' only when the body is a string and not one of
 * the native non-JSON body types (FormData, Blob, URLSearchParams, or
 * ArrayBuffer/View). Returns null when the Content-Type should not be set
 * automatically.
 *
 * This helper centralises body type detection to reduce cognitive complexity
 * in the request code while preserving existing behaviour exactly.
 *
 * @param body - The raw `RequestInit.body` value provided by the caller.
 * @returns The mime type string or null when no automatic header should be set.
 * @example
 * getContentTypeForBody('{"a":1}') //=> 'application/json'
 * getContentTypeForBody(new FormData()) //=> null
 */
function getContentTypeForBody(body: unknown): string | null {
  if (!isStringBody(body)) return null;
  if (isFormData(body)) return null;
  if (isBlob(body)) return null;
  if (isUrlEncoded(body)) return null;
  if (isBinary(body)) return null;
  return 'application/json';
}

// biome-ignore lint/complexity/noExcessiveCognitiveComplexity: complexity reduced via helper function
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
