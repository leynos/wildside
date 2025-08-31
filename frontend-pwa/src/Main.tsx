/**
 * @file Entrypoint that mounts the PWA with a shared query client.
 */
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import React from 'react';
import { createRoot } from 'react-dom/client';
import '@app/tokens/css/variables.css';
import './index.css';
import { App } from './app/App';

const QUERY_CLIENT = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 60_000,
      refetchOnWindowFocus: false,
      retry: 2,
    },
  },
});
const ROOT_ELEM = document.getElementById('root');
if (!ROOT_ELEM) throw new Error('#root element not found');

createRoot(ROOT_ELEM).render(
  <React.StrictMode>
    <QueryClientProvider client={QUERY_CLIENT}>
      <App />
    </QueryClientProvider>
  </React.StrictMode>,
);
