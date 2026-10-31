/** @file Contracts for the bounded fetcher diagnostics seam. */
import { afterEach, describe, expect, it, mock } from 'bun:test';

import {
  describeOperation,
  elapsedSince,
  emitFetchDiagnostic,
  type FetchDiagnosticEvent,
  requestMethod,
  setFetchDiagnostics,
  statusClassOf,
} from './diagnostics';

const sampleEvent: FetchDiagnosticEvent = {
  kind: 'request',
  method: 'GET',
  operation: '/api/users',
};

afterEach(() => {
  setFetchDiagnostics(undefined);
  mock.restore();
});

describe('describeOperation', () => {
  it('drops the origin, query string, and fragment', () => {
    expect(describeOperation(new URL('https://api.example/api/users?expand=roles#top'))).toBe(
      '/api/users',
    );
  });

  it.each([
    ['numeric', '/api/users/42', '/api/users/:id'],
    ['uuid', '/api/users/0f8fad5b-d9cb-469f-a165-70867728950e', '/api/users/:id'],
    ['long hexadecimal', '/api/objects/0123456789abcdef01', '/api/objects/:id'],
  ])('masks a %s identifier segment', (_label, path, expected) => {
    expect(describeOperation(new URL(path, 'http://host'))).toBe(expected);
  });

  it('keeps readable route segments', () => {
    expect(describeOperation(new URL('/api/walk-sessions/current', 'http://host'))).toBe(
      '/api/walk-sessions/current',
    );
  });
});

describe('statusClassOf', () => {
  it.each([
    [200, '2xx'],
    [204, '2xx'],
    [404, '4xx'],
    [503, '5xx'],
  ])('maps %i to %s', (status, expected) => {
    expect(statusClassOf(status)).toBe(expected);
  });
});

describe('requestMethod', () => {
  it('defaults to GET and upper-cases a caller method', () => {
    expect(requestMethod()).toBe('GET');
    expect(requestMethod('post')).toBe('POST');
  });
});

describe('elapsedSince', () => {
  it('never reports a negative duration', () => {
    expect(elapsedSince(performance.now() + 60_000)).toBe(0);
  });
});

describe('emitFetchDiagnostic', () => {
  it('emits nothing while no sink is installed', () => {
    expect(() => emitFetchDiagnostic(sampleEvent)).not.toThrow();
  });

  it('forwards events to the installed sink', () => {
    const events: FetchDiagnosticEvent[] = [];
    setFetchDiagnostics((event) => events.push(event));

    emitFetchDiagnostic(sampleEvent);

    expect(events).toEqual([sampleEvent]);
  });

  it('swallows sink faults so request outcomes are unaffected', () => {
    const sink = mock(() => {
      throw new Error('sink failed');
    });
    setFetchDiagnostics(sink);

    expect(() => emitFetchDiagnostic(sampleEvent)).not.toThrow();
    expect(sink).toHaveBeenCalledTimes(1);
  });

  it('stops emitting once the sink is cleared', () => {
    const events: FetchDiagnosticEvent[] = [];
    setFetchDiagnostics((event) => events.push(event));
    emitFetchDiagnostic(sampleEvent);
    setFetchDiagnostics(undefined);

    emitFetchDiagnostic(sampleEvent);

    // The event recorded before clearing survives; nothing is added after.
    expect(events).toEqual([sampleEvent]);
  });
});
