/**
 * @file Bounded diagnostics for the shared API fetcher.
 *
 * The fetcher reports what happened at the HTTP boundary without reporting
 * what was sent. An event carries the method, a bounded route template, the
 * elapsed time, the status class, a bounded failure category, and the backend
 * trace identifier. It never carries a URL, query string, body, credential, or
 * free-form error message.
 *
 * Route templates come from the declared table below, mirroring the paths in
 * `spec/openapi.json`: a path either fits one template exactly or reports the
 * single `/unknown` fallback, and a literal template takes precedence over a
 * `:id` template that would also fit. The set of labels a consumer can ever see is
 * therefore finite and reviewable, and no label can carry a caller-supplied
 * name, address, token, or other path-embedded personal data. Add a template
 * here when the API gains one, otherwise its routes report `/unknown`.
 *
 * The same reasoning bounds the method and the backend trace identifier: both
 * arrive from the caller or the network, so both are reduced to a known verb or
 * a capped, well-formed identifier rather than forwarded verbatim.
 *
 * The application installs a sink with {@link setFetchDiagnostics}. While no
 * sink is installed the fetcher still runs and simply emits nothing.
 */

/** Failure categories the fetcher can report without exposing request data. */
export type FetchFailureCategory = 'transport' | 'http' | 'json' | 'schema';

/** One bounded event emitted by the shared fetcher. */
export interface FetchDiagnosticEvent {
  /** Stage of the logical request this event describes. */
  readonly kind: 'request' | 'response' | 'failure';
  /** Upper-case HTTP method; `GET` by default, `OTHER` for an unknown verb. */
  readonly method: string;
  /** Declared route template, or `/unknown` when the path is not listed. */
  readonly operation: string;
  /** Whole milliseconds since the logical request started. */
  readonly durationMs?: number;
  /** Status class such as `2xx` or `5xx`, when a response arrived. */
  readonly statusClass?: string;
  /** Bounded failure category; present only on a `failure` event. */
  readonly failureCategory?: FetchFailureCategory;
  /** Backend `trace-id` header value, when the response carried one. */
  readonly traceId?: string;
}

/** Receives bounded fetcher diagnostics. */
export type FetchDiagnosticsSink = (event: FetchDiagnosticEvent) => void;

let sink: FetchDiagnosticsSink | undefined;

/** Install or clear the application-wide diagnostics sink. */
export const setFetchDiagnostics = (next: FetchDiagnosticsSink | undefined): void => {
  sink = next;
};

/**
 * Publish one bounded event to the installed sink.
 *
 * A sink fault must never change the outcome of a request, so sink failures
 * are swallowed here rather than reaching the caller.
 */
export const emitFetchDiagnostic = (event: FetchDiagnosticEvent): void => {
  if (!sink) return;
  try {
    sink(event);
  } catch {
    // Diagnostics are best effort; request semantics take precedence.
  }
};

/** Methods reported verbatim; anything else reports `OTHER`. */
const knownMethods: ReadonlySet<string> = new Set([
  'GET',
  'HEAD',
  'POST',
  'PUT',
  'PATCH',
  'DELETE',
  'OPTIONS',
  'TRACE',
  'CONNECT',
]);

/**
 * Bound the caller's method to a known HTTP verb.
 *
 * The method arrives from the caller, so it is untrusted input and must not
 * become an arbitrary metric label. Omitting it means `GET`, which is what
 * `fetch` itself assumes; a value outside the known verbs reports `OTHER`.
 *
 * @example
 * requestMethod('post');     // 'POST'
 * requestMethod('break-all'); // 'OTHER'
 */
export const requestMethod = (method?: string): string => {
  const normalized = (method ?? 'GET').toUpperCase();
  return knownMethods.has(normalized) ? normalized : 'OTHER';
};

/** Monotonic millisecond clock used to time fetcher operations. */
export type FetchClock = () => number;

const platformClock: FetchClock = () => performance.now();

let clock: FetchClock = platformClock;

/**
 * Install or restore the clock that times fetcher operations.
 *
 * The fetcher measures logical requests with `performance.now` by default.
 * Tests and embeddings install a deterministic clock instead, so a timing
 * assertion does not depend on the host's wall clock. Passing `undefined`
 * restores the platform clock.
 *
 * @example
 * let nowMs = 0;
 * setFetchClock(() => nowMs);
 * nowMs = 40;
 * elapsedSince(0); // 40
 * setFetchClock();
 */
export const setFetchClock = (next?: FetchClock): void => {
  clock = next ?? platformClock;
};

/** Current reading of the installed clock, in whole milliseconds. */
export const fetchClockNow = (): number => clock();

/** Whole milliseconds since `startedAt` on the installed clock, never negative. */
export const elapsedSince = (startedAt: number): number =>
  Math.max(0, Math.round(clock() - startedAt));

/** Map an HTTP status to its class label, for example `2xx`. */
export const statusClassOf = (status: number): string => `${Math.trunc(status / 100)}xx`;

/**
 * Route templates the API serves, mirroring `spec/openapi.json`.
 *
 * A `:id` position matches exactly one non-empty segment. Members of a
 * collection are listed alongside the collection itself because the generated
 * clients address individual resources. Nothing is derived by masking: a path
 * either fits a listed template or reports the fallback, so the labels a
 * consumer can see stay finite however the caller shapes the URL.
 */
const knownRouteTemplates: readonly string[] = [
  '/api/v1/admin/enrichment/provenance',
  '/api/v1/catalogue/descriptors',
  '/api/v1/catalogue/explore',
  '/api/v1/login',
  '/api/v1/offline/bundles',
  '/api/v1/offline/bundles/:id',
  '/api/v1/routes',
  '/api/v1/routes/:id',
  '/api/v1/routes/:id/annotations',
  '/api/v1/routes/:id/notes',
  '/api/v1/routes/:id/progress',
  '/api/v1/users',
  '/api/v1/users/:id',
  '/api/v1/users/me',
  '/api/v1/users/me/interests',
  '/api/v1/users/me/preferences',
  '/api/v1/walk-sessions',
  '/api/v1/walk-sessions/:id',
  '/health/live',
  '/health/ready',
];

/** Operation reported for any path the template table does not list. */
const fallbackOperation = '/unknown';

/** Comparison form of a template or path; the leading empty segment is kept. */
const segmentsOf = (path: string): readonly string[] => path.split('/');

/** Whether one declared segment accepts the corresponding path segment. */
const segmentAccepts = (declared: string, actual: string): boolean =>
  declared === ':id' ? actual !== '' : declared === actual;

/** Whether a path fits a template exactly, with one segment per `:id`. */
const fitsTemplate = (template: readonly string[], path: readonly string[]): boolean =>
  template.length === path.length &&
  template.every((declared, index) => segmentAccepts(declared, path[index]));

const templateSegments: readonly (readonly string[])[] = knownRouteTemplates.map((template) =>
  segmentsOf(template),
);

/** Literal templates, which win over a `:id` template that would also fit. */
const literalTemplates: ReadonlySet<string> = new Set(knownRouteTemplates);

/** Path segments for matching, with trailing slashes ignored. */
const pathSegments = (url: URL): readonly string[] => {
  const trimmed = url.pathname.replace(/\/+$/, '');
  return segmentsOf(trimmed === '' ? '/' : trimmed);
};

/**
 * Derive a bounded route template from a request URL.
 *
 * Only the path is consulted: the origin, query string, and fragment are
 * dropped, and an unlisted path reports `/unknown`. Because the
 * result is always one of a declared, finite set, an operation label cannot
 * carry a caller-supplied name, address, token, or other path-embedded data.
 *
 * @example
 * describeOperation(new URL('/api/v1/users/42?expand=roles', 'http://host'));
 * // '/api/v1/users/:id'
 *
 * @example
 * describeOperation(new URL('/api/v1/users/alice@example.com', 'http://host'));
 * // '/api/v1/users/:id'
 *
 * @example
 * describeOperation(new URL('/internal/widgets', 'http://host'));
 * // '/unknown'
 */
export const describeOperation = (url: URL): string => {
  const path = pathSegments(url);
  // A literal template wins over a `:id` template that would also fit, so
  // `/api/v1/users/me` reports itself rather than the per-user route.
  const exact = path.join('/');
  if (literalTemplates.has(exact)) return exact;
  const match = templateSegments.findIndex((template) => fitsTemplate(template, path));
  return match === -1 ? fallbackOperation : knownRouteTemplates[match];
};

/** Well-formed correlation identifiers only, up to 128 characters. */
const traceIdPattern = /^[\w.:-]{1,128}$/;

/**
 * Bound a backend correlation identifier to a safe, finite shape.
 *
 * The header arrives from the network, so it is untrusted. A value that is
 * empty, longer than 128 characters, or contains anything but letters, digits,
 * `_`, `.`, `:`, or `-` is discarded rather than truncated: a silently
 * shortened identifier would no longer match the backend's own records.
 *
 * @example
 * boundTraceId('trace-abc-123'); // 'trace-abc-123'
 * boundTraceId('  ');            // undefined
 */
export const boundTraceId = (value: string | null | undefined): string | undefined => {
  const trimmed = value?.trim();
  if (!trimmed || !traceIdPattern.test(trimmed)) return undefined;
  return trimmed;
};
