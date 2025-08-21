/**
 * @file Entrypoint that mounts the PWA with a shared query client.
 */
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import React from 'react';
import { createRoot } from 'react-dom/client';
import '@app/tokens/css/variables.css';
import './index.css';
import { App } from './app/App';

const qc = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 60_000,
      refetchOnWindowFocus: false,
    },
  },
});
const rootElem = document.getElementById('root');
if (!rootElem) throw new Error('#root element not found');

createRoot(rootElem).render(
  <React.StrictMode>
    <QueryClientProvider client={qc}>
      <App />
    </QueryClientProvider>
  </React.StrictMode>,
);
