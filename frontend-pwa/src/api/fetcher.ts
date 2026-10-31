/**
 * @file Fetch helper that augments requests with common defaults.
 */

import { z } from 'zod';

import {
  describeOperation,
  elapsedSince,
  emitFetchDiagnostic,
  requestMethod,
  statusClassOf,
} from './diagnostics';

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
// JSON scalars are legal wire bodies even though they are not structures.
const jsonScalarSchema = z.union([z.number(), z.boolean()]);

/** Predicate: does this string already hold a valid JSON document? */
const isJsonString = (value: string): boolean => {
  try {
    return jsonValueSchema.safeParse(JSON.parse(value)).success;
  } catch {
    return false;
  }
};
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
 * Plain objects, arrays, and JSON scalars are serialized; a string body is
 * forwarded untouched and labelled JSON only when it already parses as JSON.
 */
const normalizeBody = (body: FetchBody | undefined): NormalizedBody => {
  const emptyBody = emptyBodySchema.safeParse(body);
  if (emptyBody.success) return { body: emptyBody.data, isJson: false };

  const nativeBody = nativeBodySchema.safeParse(body);
  if (nativeBody.success) return { body: nativeBody.data, isJson: false };

  if (isPlainObjectLike(body) || jsonScalarSchema.safeParse(body).success) {
    return { body: stringBodySchema.parse(JSON.stringify(body)), isJson: true };
  }

  const stringBody = stringBodySchema.safeParse(body);
  if (stringBody.success) {
    return { body: stringBody.data, isJson: isJsonString(stringBody.data) };
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

  if (bodyInfo.isJson && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json; charset=utf-8');
  }
  return headers;
};

type ErrorDetail = JsonValue | string;

/**
 * Read the error body as JSON when the response advertises it, otherwise as
 * text. A body that cannot be decoded becomes an absent detail rather than
 * masking the HTTP status.
 */
const parseErrorDetail = async (res: Response): Promise<ErrorDetail | undefined> => {
  const contentType = (res.headers.get('content-type') ?? '').trim().toLowerCase();
  try {
    if (!contentType.includes('json')) return await res.text();
    return jsonValueSchema.parse(await res.json());
  } catch {
    return undefined;
  }
};

/**
 * Decide whether a successful response carries a JSON document.
 *
 * `204` and `205` responses never carry a body, and an explicit zero length
 * marks an empty body even when the content type claims JSON.
 */
const shouldParseJson = (res: Response): boolean => {
  if (res.status === 204 || res.status === 205) return false;
  const contentType = (res.headers.get('content-type') ?? '').trim().toLowerCase();
  const contentLength = res.headers.get('content-length');
  return contentType.includes('json') && contentLength !== '0';
};

interface FetchContext {
  readonly method: string;
  readonly operation: string;
  readonly startedAt: number;
}

/**
 * Perform the request, reporting transport failures without masking them.
 *
 * `fetch` rejects with a network error that callers still need to observe, so
 * the original error is rethrown after the failure is reported.
 */
const fetchGuarded = async (
  request: () => Promise<Response>,
  context: FetchContext,
): Promise<Response> => {
  try {
    return await request();
  } catch (error) {
    emitFetchDiagnostic({
      kind: 'failure',
      method: context.method,
      operation: context.operation,
      durationMs: elapsedSince(context.startedAt),
      failureCategory: 'transport',
    });
    throw error;
  }
};

/**
 * Fetch a JSON response after validating its wire representation.
 *
 * The repository's ai-deslop policy rejects `unknown` returns, so this
 * boundary validates the payload once and hands callers a named JSON type.
 */
export const customFetch = async (
  input: string,
  init?: FetchRequestInit,
): Promise<JsonValue | undefined> => {
  const url = new URL(input, apiBase());
  const operation = describeOperation(url);
  const method = requestMethod(init?.method);
  const startedAt = performance.now();
  emitFetchDiagnostic({ kind: 'request', method, operation });
  const context = { method, operation, startedAt };

  const { body, isJson } = normalizeBody(init?.body);
  const headers = defaultHeaders(init, { isJson });
  const res = await fetchGuarded(
    () => fetch(url, { credentials: 'include', ...init, body, headers }),
    context,
  );

  const timing = {
    method,
    operation,
    durationMs: elapsedSince(startedAt),
    statusClass: statusClassOf(res.status),
    traceId: res.headers.get('trace-id') ?? undefined,
  };

  if (!res.ok) {
    emitFetchDiagnostic({ ...timing, kind: 'failure', failureCategory: 'http' });
    const detail = await parseErrorDetail(res);
    const suffix = detail ? `: ${JSON.stringify(detail)}` : '';
    throw new Error(`${res.status} ${res.statusText} ${res.url}${suffix}`, { cause: detail });
  }

  if (!shouldParseJson(res)) {
    emitFetchDiagnostic({ ...timing, kind: 'response' });
    return;
  }

  try {
    const payload = jsonValueSchema.parse(await res.json());
    emitFetchDiagnostic({ ...timing, kind: 'response' });
    return payload;
  } catch (error) {
    emitFetchDiagnostic({ ...timing, kind: 'failure', failureCategory: 'json' });
    throw error;
  }
};

/**
 * Orval-compatible mutator that adapts {@link customFetch} to the generator's
 * call shape.
 *
 * Orval's fetch client invokes its configured mutator as
 * `mutator<ResponseType>(input, init)`, so the mutator must accept a response
 * type argument. The generated types mirror the OpenAPI schema the backend
 * serves, while `customFetch` still enforces the transport contract: non-2xx
 * responses reject, and successful payloads must be valid JSON. The assertion
 * only records the generator's compile-time view of that payload.
 *
 * Use {@link customFetch} directly in handwritten code; this adapter exists
 * so generated clients keep compiling.
 *
 * @example
 * // Generated clients call this as `customFetchOrval<ResponseType>(url, init)`.
 * const users = await customFetchOrval<Array<{ id: string }>>('/api/users');
 */
export const customFetchOrval = async <ResponseType>(
  input: string,
  init?: FetchRequestInit,
): Promise<ResponseType> => {
  const payload = await customFetch(input, init);
  // SAFETY: `customFetch` has already validated the payload as JSON, and the
  // generated `ResponseType` mirrors the OpenAPI schema that same backend
  // serves. No runtime check can reconcile the two cheaply here, so this cast
  // records the generator's view rather than widening `customFetch` itself.
  return payload as ResponseType;
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
  const startedAt = performance.now();
  const operation = describeOperation(new URL(input, apiBase()));
  const method = requestMethod(init?.method);

  const result = schema.safeParse(await customFetch(input, init));
  if (result.success) return result.data;

  emitFetchDiagnostic({
    kind: 'failure',
    method,
    operation,
    durationMs: elapsedSince(startedAt),
    failureCategory: 'schema',
  });
  throw result.error;
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
  const startedAt = performance.now();
  const operation = describeOperation(new URL(input, apiBase()));
  const method = requestMethod(init?.method);

  const result = schema.safeParse(await customFetch(input, init));
  if (!result.success) {
    emitFetchDiagnostic({
      kind: 'failure',
      method,
      operation,
      durationMs: elapsedSince(startedAt),
      failureCategory: 'schema',
    });
  }
  return result;
};
