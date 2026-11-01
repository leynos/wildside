/**
 * @file Entrypoint that mounts the PWA with a shared query client.
 */
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import React from 'react';
import { createRoot } from 'react-dom/client';
import './index.css';
import { type FetchDiagnosticEvent, setFetchDiagnostics } from './api/diagnostics';
import { createFetchMetrics, publishFetchMetrics } from './api/fetch-metrics';
import { App } from './app/App';

/** Bounded in-memory consumer, published for devtools and tests to read. */
const fetchMetrics = createFetchMetrics();
publishFetchMetrics(fetchMetrics);

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
const rootElem = document.querySelector('#root');
if (!rootElem) throw new Error('#root element not found');

createRoot(rootElem).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  </React.StrictMode>,
);
