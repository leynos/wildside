/**
 * @file Bounded diagnostics for the shared API fetcher.
 *
 * The fetcher reports what happened at the HTTP boundary without reporting
 * what was sent. An event carries the method, a masked route template, the
 * elapsed time, the status class, a bounded failure category, and the backend
 * trace identifier. It never carries a URL, query string, body, credential, or
 * free-form error message, so the same record can be forwarded to any
 * destination without a privacy review per sink.
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
  /** Upper-case HTTP method; `GET` when the caller did not set one. */
  readonly method: string;
  /** Route template with identifier-shaped segments masked and no query. */
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

/** Upper-case the request method, defaulting to `GET`. */
export const requestMethod = (method?: string): string => (method ?? 'GET').toUpperCase();

/** Whole milliseconds since `startedAt`, never negative. */
export const elapsedSince = (startedAt: number): number =>
  Math.max(0, Math.round(performance.now() - startedAt));

/** Map an HTTP status to its class label, for example `2xx`. */
export const statusClassOf = (status: number): string => `${Math.trunc(status / 100)}xx`;

const numericSegment = /^\d+$/;
const uuidSegment = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const longIdentifierSegment = /^[0-9a-f]{16,}$/i;

/**
 * Mask identifier-shaped path segments.
 *
 * Numeric identifiers, UUIDs, and long hexadecimal identifiers become `:id`,
 * which keeps the template bounded and free of request data.
 */
const boundSegment = (segment: string): string =>
  numericSegment.test(segment) || uuidSegment.test(segment) || longIdentifierSegment.test(segment)
    ? ':id'
    : segment;

/**
 * Derive a bounded route template from a request URL.
 *
 * Only the path survives: the origin, query string, and fragment are dropped
 * and identifier-shaped segments are masked.
 *
 * @example
 * describeOperation(new URL('/api/users/42?expand=roles', 'http://host'));
 * // '/api/users/:id'
 */
export const describeOperation = (url: URL): string =>
  url.pathname
    .split('/')
    .map((segment) => boundSegment(segment))
    .join('/') || '/';
