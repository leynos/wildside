/**
 * @file Entrypoint that mounts the PWA with a shared query client.
 */
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import React from 'react';
import { createRoot } from 'react-dom/client';
import './index.css';
import { type FetchDiagnosticEvent, setFetchDiagnostics } from './api/diagnostics';
import { App } from './app/App';

/**
 * Report bounded fetcher failures through the global error reporting channel.
 *
 * The PWA has no telemetry destination yet, so failures go to the browser's
 * own reporting pipeline: `reportError` raises a global `error` event that
 * devtools, window error handlers, and any future telemetry adapter observe.
 * The event carries a masked route template and bounded categories, never a
 * URL, query string, body, credential, or token.
 */
const reportFetchFailure = (event: FetchDiagnosticEvent): void => {
  if (event.kind !== 'failure') return;
  reportError(new Error(`Fetch failure: ${event.failureCategory ?? 'unknown'}`, { cause: event }));
};
setFetchDiagnostics(reportFetchFailure);

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
