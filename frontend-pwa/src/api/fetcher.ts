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
const nullBodySchema = z.null();
const undefinedBodySchema = z.undefined();

const apiBase = (): string => import.meta.env.VITE_API_BASE ?? 'http://localhost:8080';

const isFormData = (body: FetchBody | undefined): body is FormData => body instanceof FormData;
const isBlob = (body: FetchBody | undefined): body is Blob => body instanceof Blob;
const isUrlEncoded = (body: FetchBody | undefined): body is URLSearchParams =>
  body instanceof URLSearchParams;
const isBinary = (
  body: FetchBody | undefined,
): body is ArrayBuffer | ArrayBufferView<ArrayBuffer> =>
  body instanceof ArrayBuffer || ArrayBuffer.isView(body);

/** Predicate: treat POJOs and arrays as JSON serializable structures. */
const isPlainObjectLike = (value: FetchBody | undefined): value is JsonObject | JsonValue[] =>
  Array.isArray(value) || (Boolean(value) && Object.getPrototypeOf(value) === Object.prototype);

/** Heuristic for strings that look like JSON payloads. */
const looksLikeJsonString = (text: string): boolean => {
  const trimmed = text.trim();
  return (
    (trimmed.startsWith('{') && trimmed.endsWith('}')) ||
    (trimmed.startsWith('[') && trimmed.endsWith(']'))
  );
};

/**
 * Normalize the request body for JSON APIs while preserving native body types.
 * Plain objects and arrays are serialized; string bodies remain untouched.
 */
const normalizeBody = (body: FetchBody | undefined): NormalizedBody => {
  if (undefinedBodySchema.safeParse(body).success) return { isJson: false };

  const nullBody = nullBodySchema.safeParse(body);
  if (nullBody.success) return { body: nullBody.data, isJson: false };

  if (isFormData(body) || isBlob(body) || isUrlEncoded(body) || isBinary(body)) {
    return { body, isJson: false };
  }

  if (isPlainObjectLike(body)) {
    return { body: stringBodySchema.parse(JSON.stringify(body)), isJson: true };
  }

  const stringBody = stringBodySchema.safeParse(body);
  if (stringBody.success) {
    return { body: stringBody.data, isJson: looksLikeJsonString(stringBody.data) };
  }

  const numericBody = z.union([z.number(), z.boolean()]).safeParse(body);
  if (numericBody.success) return { body: String(numericBody.data), isJson: false };

  const streamBody = z.instanceof(ReadableStream).safeParse(body);
  if (streamBody.success) return { body: streamBody.data, isJson: false };

  return { body: String(body), isJson: false };
};

/** Build default headers and attach JSON content type when appropriate. */
const defaultHeaders = (
  init: FetchRequestInit | undefined,
  bodyInfo: Pick<NormalizedBody, 'isJson'>,
): Headers => {
  const headers = new Headers(init?.headers);
  if (!headers.has('Accept')) headers.set('Accept', 'application/json');

  const rawBody = init?.body;
  if (!headers.has('Content-Type') && (bodyInfo.isJson || isStringBody(rawBody))) {
    headers.set('Content-Type', 'application/json; charset=utf-8');
  }
  return headers;
};

function isStringBody(body: FetchBody | undefined): boolean {
  return stringBodySchema.safeParse(body).success;
}

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
