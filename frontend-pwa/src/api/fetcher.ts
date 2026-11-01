/**
 * @file JSON boundary between the PWA and its backend.
 *
 * Every request passes through one place so that transport defaults, payload
 * validation, and diagnostics stay consistent. `normalizeBody` serializes plain
 * objects, arrays, and JSON scalars while forwarding strings and native body
 * types untouched; `defaultHeaders` adds the JSON content type only for bodies
 * that were serialized. Responses must be valid JSON when the server advertises
 * a JSON body, and `customFetchParsed`/`customFetchParsedSafe` validate the
 * decoded payload against a caller-supplied schema. Failures and timings are
 * reported through the bounded seam in `diagnostics.ts`; the timing clock is
 * injectable there so tests can assert durations without the host clock.
 */

import { z } from 'zod';

import {
  boundTraceId,
  describeOperation,
  elapsedSince,
  emitFetchDiagnostic,
  fetchClockNow,
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

/** Resolve the API origin from the build-time environment, defaulting locally. */
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
  readonly signal?: AbortSignal;
}

/**
 * Predicate: did the caller abort this logical request?
 *
 * A cancellation is a deliberate caller action, not a fault, so both the
 * transport and body-read boundaries use this predicate to keep it out of the
 * failure counts and the global error pipeline. A rejection is matched either
 * by its `AbortError` name or by identity with the caller's signal reason, so a
 * wrapper that rethrows the reason is still classified as a cancellation.
 * `TimeoutError` is a real transport failure and deliberately does not match.
 *
 * @example
 * isAbortError(new DOMException('aborted', 'AbortError'), undefined); // true
 * isAbortError(new DOMException('timed out', 'TimeoutError'), undefined); // false
 */
const isAbortError = (error: Error, signal: AbortSignal | undefined): boolean => {
  if (error.name === 'TimeoutError') return false;
  return error.name === 'AbortError' || (signal?.aborted === true && error === signal.reason);
};

/**
 * Perform the request, reporting transport failures without masking them.
 *
 * `fetch` rejects with a network error that callers still need to observe, so
 * the original error is rethrown after the failure is reported. A caller's own
 * abort is not a transport fault: `customFetch` forwards `init.signal`, so
 * excluding `AbortError` keeps a deliberate cancellation out of the failure
 * counts and the global error pipeline. A `TimeoutError` is still reported,
 * because a timeout is a genuine transport failure the caller did not choose.
 */
const fetchGuarded = async (
  request: () => Promise<Response>,
  context: FetchContext,
): Promise<Response> => {
  try {
    return await request();
  } catch (error) {
    if (error instanceof Error && isAbortError(error, context.signal)) throw error;
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

interface PreparedRequest {
  readonly url: URL;
  readonly context: FetchContext;
  readonly body: BodyInit | null | undefined;
  readonly headers: Headers;
}

/** Record the request start, then normalize its body and headers. */
const prepareRequest = (input: string, init: FetchRequestInit | undefined): PreparedRequest => {
  const url = new URL(input, apiBase());
  const operation = describeOperation(url);
  const method = requestMethod(init?.method);
  const startedAt = fetchClockNow();
  emitFetchDiagnostic({ kind: 'request', method, operation });

  const { body, isJson } = normalizeBody(init?.body);
  return {
    url,
    context: { method, operation, startedAt, signal: init?.signal ?? undefined },
    body,
    headers: defaultHeaders(init, { isJson }),
  };
};

interface ResponseTiming {
  readonly method: string;
  readonly operation: string;
  readonly durationMs: number;
  readonly statusClass: string;
  readonly traceId?: string;
}

/**
 * Describe a response in the bounded terms a diagnostics sink may receive.
 *
 * Duration is measured here, at emission time, so a success or failure event
 * covers body consumption and payload validation as well as the request.
 */
const responseTiming = (res: Response, context: FetchContext): ResponseTiming => ({
  method: context.method,
  operation: context.operation,
  durationMs: elapsedSince(context.startedAt),
  statusClass: statusClassOf(res.status),
  // The header is network input, so it is bounded before it is retained.
  traceId: boundTraceId(res.headers.get('trace-id')),
});

/** Report an HTTP failure, then reject with the status and error detail. */
const rejectHttpFailure = async (res: Response, context: FetchContext): Promise<never> => {
  emitFetchDiagnostic({
    ...responseTiming(res, context),
    kind: 'failure',
    failureCategory: 'http',
  });
  const detail = await parseErrorDetail(res);
  const suffix = detail ? `: ${JSON.stringify(detail)}` : '';
  throw new Error(`${res.status} ${res.statusText} ${res.url}${suffix}`, { cause: detail });
};

/**
 * Decode and validate a JSON payload, reporting decoding faults as `json`.
 *
 * An abort can also surface here, while the body is being read; the caller
 * chose that cancellation, so it is rethrown without becoming a `json` fault.
 */
const readJsonPayload = async (res: Response, context: FetchContext): Promise<JsonValue> => {
  try {
    const payload = jsonValueSchema.parse(await res.json());
    emitFetchDiagnostic({ ...responseTiming(res, context), kind: 'response' });
    return payload;
  } catch (error) {
    if (!(error instanceof Error && isAbortError(error, context.signal))) {
      emitFetchDiagnostic({
        ...responseTiming(res, context),
        kind: 'failure',
        failureCategory: 'json',
      });
    }
    throw error;
  }
};

/** Classify a response and resolve with its validated payload when present. */
const handleResponse = async (
  res: Response,
  context: FetchContext,
): Promise<JsonValue | undefined> => {
  if (!res.ok) return rejectHttpFailure(res, context);
  if (!shouldParseJson(res)) {
    emitFetchDiagnostic({ ...responseTiming(res, context), kind: 'response' });
    return undefined;
  }
  return readJsonPayload(res, context);
};

interface FetchOutcome {
  readonly payload: JsonValue | undefined;
  readonly timing: ResponseTiming;
  readonly startedAt: number;
}

/**
 * Report a caller-schema failure against the response that carried the payload.
 *
 * The transport already reported its own completion, so this event carries the
 * same status class and trace identifier plus a duration that includes schema
 * validation, letting a consumer pair both events for one logical operation.
 */
const emitSchemaFailure = (outcome: FetchOutcome): void => {
  emitFetchDiagnostic({
    ...outcome.timing,
    durationMs: elapsedSince(outcome.startedAt),
    kind: 'failure',
    failureCategory: 'schema',
  });
};

/**
 * Fetch a JSON response and return it with its bounded response timing.
 *
 * Callers that validate the payload further need the status class and trace
 * identifier of the response they validated, so both travel with the payload.
 */
const customFetchOutcome = async (
  input: string,
  init?: FetchRequestInit,
): Promise<FetchOutcome> => {
  const { url, context, body, headers } = prepareRequest(input, init);
  const res = await fetchGuarded(
    () => fetch(url, { credentials: 'include', ...init, body, headers }),
    context,
  );

  const payload = await handleResponse(res, context);
  return {
    payload,
    timing: responseTiming(res, context),
    startedAt: context.startedAt,
  };
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
  const { payload } = await customFetchOutcome(input, init);
  return payload;
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
  const outcome = await customFetchOutcome(input, init);
  const result = schema.safeParse(outcome.payload);
  if (result.success) return result.data;

  emitSchemaFailure(outcome);
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
  const outcome = await customFetchOutcome(input, init);
  const result = schema.safeParse(outcome.payload);
  if (!result.success) emitSchemaFailure(outcome);
  return result;
};
