/**
 * @file Fetch helper that augments requests with common defaults.
 */

import { z } from 'zod';

type JsonValue = string | number | boolean | null | JsonObject | JsonValue[];
interface JsonObject {
  [key: string]: JsonValue;
}

type FetchBody = BodyInit | JsonValue | null;
type FetchRequestInit = Omit<RequestInit, 'body'> & { body?: FetchBody };

interface NormalizedBody {
  body?: BodyInit | null;
  isJson: boolean;
}

const jsonValueSchema: z.ZodType<JsonValue> = z.lazy(() =>
  z.union([
    z.string(),
    z.number(),
    z.boolean(),
    z.null(),
    z.array(jsonValueSchema),
    z.record(z.string(), jsonValueSchema),
  ]),
);

const stringBodySchema = z.string();
const emptyBodySchema = z.union([z.null(), z.undefined()]);
const nativeBodySchema = z.union([
  z.instanceof(FormData),
  z.instanceof(Blob),
  z.instanceof(URLSearchParams),
  z.instanceof(ArrayBuffer),
  z.custom<ArrayBufferView<ArrayBuffer>>((value) => ArrayBuffer.isView(value)),
  z.instanceof(ReadableStream),
]);

const apiBase = (): string => import.meta.env.VITE_API_BASE ?? 'http://localhost:8080';

/** Predicate: treat POJOs and arrays as JSON serializable structures. */
const isPlainObjectLike = (value: FetchBody | undefined): value is JsonObject | JsonValue[] =>
  Array.isArray(value) || (Boolean(value) && Object.getPrototypeOf(value) === Object.prototype);

/**
 * Normalize the request body for JSON APIs while preserving native body types.
 * Plain objects and arrays are serialized; string bodies remain untouched.
 */
const normalizeBody = (body: FetchBody | undefined): NormalizedBody => {
  const emptyBody = emptyBodySchema.safeParse(body);
  if (emptyBody.success) return { body: emptyBody.data, isJson: false };

  const nativeBody = nativeBodySchema.safeParse(body);
  if (nativeBody.success) return { body: nativeBody.data, isJson: false };

  if (isPlainObjectLike(body)) {
    return { body: stringBodySchema.parse(JSON.stringify(body)), isJson: true };
  }

  return { body: String(body), isJson: false };
};

/** Build default headers and attach JSON content type when appropriate. */
const defaultHeaders = (
  init: FetchRequestInit | undefined,
  bodyInfo: Pick<NormalizedBody, 'isJson'>,
): Headers => {
  const headers = new Headers(init?.headers);
  if (!headers.has('Accept')) headers.set('Accept', 'application/json');

  const shouldSetJsonContentType =
    bodyInfo.isJson || stringBodySchema.safeParse(init?.body).success;
  if (shouldSetJsonContentType && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json; charset=utf-8');
  }
  return headers;
};

type ErrorDetail = JsonValue | string;

const parseErrorDetail = async (res: Response): Promise<ErrorDetail | undefined> => {
  const contentType = (res.headers.get('content-type') ?? '').trim().toLowerCase();
  try {
    if (!contentType.includes('json')) return await res.text();
    return jsonValueSchema.parse(await res.json());
  } catch {
    return undefined;
  }
};

const shouldParseJson = (res: Response): boolean => {
  if (res.status === 204 || res.status === 205) return false;
  const contentType = (res.headers.get('content-type') ?? '').trim().toLowerCase();
  const contentLength = res.headers.get('content-length');
  return contentType.includes('json') && contentLength !== '0';
};

/** Fetch a JSON response after validating its wire representation. */
export const customFetch = async (
  input: string,
  init?: FetchRequestInit,
): Promise<JsonValue | undefined> => {
  const url = new URL(input, apiBase());
  const { body, isJson } = normalizeBody(init?.body);
  const headers = defaultHeaders(init, { isJson });
  const res = await fetch(url, { credentials: 'include', ...init, body, headers });

  if (!res.ok) {
    const detail = await parseErrorDetail(res);
    const suffix = detail ? `: ${JSON.stringify(detail)}` : '';
    throw new Error(`${res.status} ${res.statusText} ${res.url}${suffix}`, { cause: detail });
  }

  if (!shouldParseJson(res)) return;
  return jsonValueSchema.parse(await res.json());
};

/**
 * Fetch JSON and validate it against a provided Zod schema.
 *
 * @example
 * import { z } from 'zod';
 * const UserSchema = z.object({ id: z.string() });
 * const user = await customFetchParsed('/api/user', UserSchema);
 * user.id; // string
 */
export const customFetchParsed = async <Schema extends z.ZodTypeAny>(
  input: string,
  schema: Schema,
  init?: FetchRequestInit,
): Promise<z.output<Schema>> => {
  return schema.parse(await customFetch(input, init));
};

/**
 * Like `customFetchParsed`, but returns Zod's safe-parse result for schema
 * validation failures so callers can branch on them. Transport errors and
 * non-2xx responses still reject.
 */
export const customFetchParsedSafe = async <Schema extends z.ZodTypeAny>(
  input: string,
  schema: Schema,
  init?: FetchRequestInit,
): Promise<z.SafeParseReturnType<unknown, z.output<Schema>>> => {
  const data = await customFetch(input, init);
  return schema.safeParse(data);
};
