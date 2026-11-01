/**
 * @file Entrypoint that boots the PWA around a shared query client.
 *
 * Importing this module boots the application exactly once. Booting publishes
 * the bounded fetch-metrics consumer for devtools, installs the diagnostics
 * sink that feeds it, records one client span per logical request in a bounded
 * span log published as `__wildsideClientSpans`, forwards fetch failures to the
 * browser's global error channel through `reportError`, and renders the React
 * tree into `#root`.
 *
 * The boot step takes the document as an argument so the missing-root guard can
 * be exercised without a live DOM: a document without `#root` must fail loudly
 * rather than render into nothing.
 */
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import React from 'react';
import { createRoot } from 'react-dom/client';
import './index.css';
import { type FetchDiagnosticEvent, setFetchDiagnostics } from './api/diagnostics';
import { createFetchMetrics, publishFetchMetrics } from './api/fetch-metrics';
import { createSpanLog, publishSpanLog } from './api/span-log';
import { setSpanSink } from './api/tracing';
import { App } from './app/App';

/** Bounded in-memory consumer, published for devtools and tests to read. */
const fetchMetrics = createFetchMetrics();
publishFetchMetrics(fetchMetrics);

/** Bounded span log, published for devtools and tests to read. */
const spanLog = createSpanLog();
publishSpanLog(spanLog);
setSpanSink(spanLog.sink);

/**
 * Feed fetcher diagnostics to the application's metrics and error channel.
 *
 * Counts, latency, and failure categories stay in the bounded in-memory
 * consumer, which devtools can read through `__wildsideFetchMetrics`. Failures
 * also go through `reportError`, so the browser's global error pipeline sees
 * them; the event carries a bounded route template and categories, never a
 * URL, query string, body, credential, or token.
 */
const observeFetchDiagnostic = (event: FetchDiagnosticEvent): void => {
  fetchMetrics.sink(event);
  if (event.kind !== 'failure') return;
  reportError(new Error(`Fetch failure: ${event.failureCategory ?? 'unknown'}`, { cause: event }));
};
setFetchDiagnostics(observeFetchDiagnostic);

// Use lower camelCase for module-level consts to satisfy lint rules.
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 60_000,
      refetchOnWindowFocus: false,
      retry: 2,
    },
  },
});

/** The document surface the boot sequence needs to locate its mount point. */
export interface MountDocument {
  /** First element matching `selector`, or nothing when the document lacks one. */
  querySelector(selector: string): Element | null | undefined;
}

/**
 * Resolve the mount point, or fail loudly rather than render nowhere.
 *
 * @example
 * requireRootElement(document); // the `#root` element
 */
export const requireRootElement = (doc: MountDocument): Element => {
  const rootElem = doc.querySelector('#root');
  if (!rootElem) throw new Error('#root element not found');
  return rootElem;
};

/**
 * Render the application into the document's root element.
 *
 * @example
 * boot(document); // the PWA is mounted and the diagnostics sink is installed
 */
export const boot = (doc: MountDocument): void => {
  createRoot(requireRootElement(doc)).render(
    <React.StrictMode>
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>
    </React.StrictMode>,
  );
};

boot(document);
