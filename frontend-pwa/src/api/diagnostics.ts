/**
 * @file Bounded diagnostics for the shared API fetcher.
 *
 * The fetcher reports what happened at the HTTP boundary without reporting
 * what was sent. An event carries the method, a bounded route template, the
 * elapsed time, the status class, a bounded failure category, and the backend
 * trace identifier. It never carries a URL, query string, body, credential, or
 * free-form error message.
 *
 * Route templates are built from the vocabulary of known API resources
 * declared in `spec/openapi.json`; any segment outside that vocabulary becomes
 * `:id` when it looks like an identifier and `:other` otherwise. The result
 * has bounded cardinality and cannot carry a caller-supplied name, address, or
 * other path-embedded personal data. Add a new resource word here when the API
 * gains one, otherwise its routes collapse to `:other`.
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
 * Path words that may appear in an operation template.
 *
 * This mirrors the resource vocabulary in `spec/openapi.json`. A new endpoint
 * whose words are absent here still reports, but its segments collapse to
 * `:other`; extend this set when a resource is added to keep the template
 * readable.
 */
const knownSegments: ReadonlySet<string> = new Set([
  'admin',
  'annotations',
  'api',
  'bundles',
  'catalogue',
  'descriptors',
  'enrichment',
  'explore',
  'health',
  'interests',
  'live',
  'login',
  'me',
  'notes',
  'offline',
  'preferences',
  'progress',
  'provenance',
  'ready',
  'routes',
  'users',
  'v1',
  'walk-sessions',
]);

const numericSegment = /^\d+$/;
const uuidSegment = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const longIdentifierSegment = /^[0-9a-f]{16,}$/i;

/** Whether a path segment looks like an opaque identifier rather than a name. */
const isIdentifierSegment = (segment: string): boolean =>
  numericSegment.test(segment) || uuidSegment.test(segment) || longIdentifierSegment.test(segment);

/**
 * Bound one path segment to a template word.
 *
 * Known vocabulary survives, identifier shapes become `:id`, and anything else
 * becomes `:other`. The empty leading segment is preserved so the template
 * keeps its leading slash.
 */
const boundSegment = (segment: string): string => {
  if (segment === '') return '';
  if (knownSegments.has(segment)) return segment;
  return isIdentifierSegment(segment) ? ':id' : ':other';
};

/**
 * Derive a bounded route template from a request URL.
 *
 * Only the path survives: the origin, query string, and fragment are dropped,
 * known resource words are kept, and every other segment is masked.
 *
 * @example
 * describeOperation(new URL('/api/users/42?expand=roles', 'http://host'));
 * // '/api/users/:id'
 *
 * @example
 * describeOperation(new URL('/api/users/alice@example.com', 'http://host'));
 * // '/api/users/:other'
 */
export const describeOperation = (url: URL): string =>
  url.pathname
    .split('/')
    .map((segment) => boundSegment(segment))
    .join('/') || '/';
