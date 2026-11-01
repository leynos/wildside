/** @file Entrypoint wiring for diagnostics, metrics, and the React root. */
import { afterAll, afterEach, beforeAll, describe, expect, it, mock } from 'bun:test';
import type { ReactElement } from 'react';

import {
  emitFetchDiagnostic,
  type FetchDiagnosticEvent,
  setFetchDiagnostics,
} from './api/diagnostics';
import type { FetchMetricsSnapshot } from './api/fetch-metrics';

/** Stand-in for the DOM element the entrypoint mounts into. */
interface RootElement {
  readonly id: string;
}

/** Options accepted by the mocked `QueryClient` constructor. */
interface QueryClientOptions {
  readonly defaultOptions?: object;
}

const rootElement: RootElement = { id: 'root' };
let mountedOn: RootElement | undefined;
let rendered: ReactElement | undefined;
const reportedErrors: Error[] = [];
const reportError = mock((error: Error) => {
  reportedErrors.push(error);
});

/** Mocked application component that renders nothing. */
const MockApp = (): void => {};

/**
 * The accessor the entrypoint published, failing when it skipped publication.
 *
 * Reading through the accessor proves the diagnostics sink is wired to the
 * same metrics instance, without reaching into the entrypoint's internals.
 */
const metrics = (): FetchMetricsSnapshot => {
  const accessor = globalThis.__wildsideFetchMetrics;
  if (!accessor) throw new Error('the entrypoint did not publish the metrics accessor');
  return accessor();
};

beforeAll(async () => {
  await mock.module('react-dom/client', () => ({
    createRoot: (target: RootElement) => {
      mountedOn = target;
      return {
        render: (element: ReactElement) => {
          rendered = element;
        },
      };
    },
  }));
  await mock.module('@tanstack/react-query', () => ({
    // The mocked exports must keep the real module's PascalCase names.
    // biome-ignore lint/style/useNamingConvention: mirrors the module's export names.
    QueryClient: class {
      readonly options: QueryClientOptions;
      constructor(options: QueryClientOptions) {
        this.options = options;
      }
    },
    // biome-ignore lint/style/useNamingConvention: mirrors the module's export names.
    QueryClientProvider: ({ children }: { children: ReactElement }) => children,
  }));
  await mock.module('./app/App', () => ({
    // biome-ignore lint/style/useNamingConvention: mirrors the module's export names.
    App: MockApp,
  }));
  await mock.module('./index.css', () => ({}));

  Object.assign(globalThis, {
    document: {
      querySelector: (selector: string) => (selector === '#root' ? rootElement : undefined),
    },
    reportError,
  });

  await import('./Main');
});

afterEach(() => {
  reportError.mockClear();
  reportedErrors.length = 0;
});

afterAll(() => {
  setFetchDiagnostics(undefined);
  globalThis.__wildsideFetchMetrics = undefined;
  Reflect.deleteProperty(globalThis, 'document');
  Reflect.deleteProperty(globalThis, 'reportError');
});

const responseEvent: FetchDiagnosticEvent = {
  kind: 'response',
  method: 'GET',
  operation: '/api/users',
  durationMs: 5,
  statusClass: '2xx',
};

const failureEvent: FetchDiagnosticEvent = {
  kind: 'failure',
  method: 'GET',
  operation: '/api/users',
  durationMs: 7,
  statusClass: '5xx',
  failureCategory: 'http',
};

describe('Main entrypoint', () => {
  it('mounts the application on the root element', () => {
    expect(mountedOn).toBe(rootElement);
    expect(rendered).toBeDefined();
  });

  it('publishes the metrics snapshot accessor', () => {
    expect(globalThis.__wildsideFetchMetrics).toBeInstanceOf(Function);
  });

  it('counts requests and responses without reporting them as errors', () => {
    const before = metrics();
    emitFetchDiagnostic({ kind: 'request', method: 'GET', operation: '/api/users' });
    emitFetchDiagnostic(responseEvent);

    expect(reportError).not.toHaveBeenCalled();
    const after = metrics();
    expect(after.requests).toBe(before.requests + 1);
    expect(after.responses).toBe(before.responses + 1);
    expect(after.latencyByOperation['/api/users']).toEqual({
      count: 1,
      totalMs: 5,
      maxMs: 5,
    });
  });

  it('reports failures with the bounded event as the cause', () => {
    const before = metrics();
    emitFetchDiagnostic(failureEvent);

    expect(reportError).toHaveBeenCalledTimes(1);
    const error = reportedErrors[0];
    expect(error).toBeInstanceOf(Error);
    expect(error.message).toBe('Fetch failure: http');
    expect(error.cause).toBe(failureEvent);
    expect(metrics().failures.http).toBe(before.failures.http + 1);
  });
});
