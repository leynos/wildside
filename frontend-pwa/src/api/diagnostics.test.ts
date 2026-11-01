/** @file Contracts for the bounded fetcher diagnostics seam. */
import { afterEach, describe, expect, it, mock } from 'bun:test';
import { readFileSync } from 'node:fs';
import { z } from 'zod';

import {
  boundTraceId,
  describeOperation,
  elapsedSince,
  emitFetchDiagnostic,
  type FetchDiagnosticEvent,
  fetchClockNow,
  requestMethod,
  setFetchClock,
  setFetchDiagnostics,
  statusClassOf,
} from './diagnostics';

/** The parts of the OpenAPI document this test reads. */
const openApiPathsSchema = z.object({
  paths: z.record(z.string(), z.object({})),
});

const sampleEvent: FetchDiagnosticEvent = {
  kind: 'request',
  method: 'GET',
  operation: '/api/v1/users',
};

afterEach(() => {
  setFetchDiagnostics(undefined);
  mock.restore();
});

describe('describeOperation', () => {
  it('drops the origin, query string, and fragment', () => {
    expect(describeOperation(new URL('https://api.example/api/v1/users?expand=roles#top'))).toBe(
      '/api/v1/users',
    );
  });

  it.each([
    ['a numeric identifier', '/api/v1/users/42', '/api/v1/users/:id'],
    [
      'a UUID identifier',
      '/api/v1/routes/0f8fad5b-d9cb-469f-a165-70867728950e/notes',
      '/api/v1/routes/:id/notes',
    ],
    [
      'a long hexadecimal identifier',
      '/api/v1/offline/bundles/0123456789abcdef01',
      '/api/v1/offline/bundles/:id',
    ],
  ])('keeps the declared template for %s', (_label, path, expected) => {
    expect(describeOperation(new URL(path, 'http://host'))).toBe(expected);
  });

  it('keeps a nested declared template', () => {
    expect(describeOperation(new URL('/api/v1/users/me/interests', 'http://host'))).toBe(
      '/api/v1/users/me/interests',
    );
  });

  it('ignores trailing slashes', () => {
    expect(describeOperation(new URL('/api/v1/walk-sessions/', 'http://host'))).toBe(
      '/api/v1/walk-sessions',
    );
  });

  it.each([
    ['an unlisted resource', '/internal/widgets'],
    ['an unlisted API version', '/api/v2/users'],
    ['a resource the spec does not declare', '/api/users/42'],
    ['a path one segment too deep', '/api/v1/users/me/extra'],
  ])('collapses %s to the fixed fallback', (_label, path) => {
    expect(describeOperation(new URL(path, 'http://host'))).toBe('/unknown');
  });

  it.each([
    ['a personal-data-shaped identifier', '/api/v1/users/alice@example.com'],
    ['a token-shaped identifier', '/api/v1/users/eyJhbGciOiJIUzI1NiJ9'],
  ])('never forwards %s', (_label, path) => {
    expect(describeOperation(new URL(path, 'http://host'))).toBe('/api/v1/users/:id');
  });

  it('bounds the template set to the declared table plus one fallback', () => {
    const labels = new Set<string>();
    for (const path of ['/a', '/a/b', '/api', '/api/v1', '/health', '/api/v1/users/me/x/y/z']) {
      labels.add(describeOperation(new URL(path, 'http://host')));
    }

    expect([...labels]).toEqual(['/unknown']);
  });

  it('reports a declared template for every path in the OpenAPI spec', () => {
    const specPath = new URL('../../../spec/openapi.json', import.meta.url);
    // Only the path keys matter here, so each path item is validated as an
    // arbitrary object rather than modelling the whole document.
    const spec = openApiPathsSchema.parse(JSON.parse(readFileSync(specPath, 'utf8')));
    const declared = Object.keys(spec.paths);

    // A path the table forgets would otherwise report `/unknown`, so this
    // fails as soon as the spec grows a route the diagnostics seam cannot name.
    expect(declared.length).toBeGreaterThan(0);
    for (const path of declared) {
      const sample = path.replaceAll(/\{[^}]+\}/g, 'sample-id');
      expect(describeOperation(new URL(sample, 'http://host'))).not.toBe('/unknown');
    }
  });
});

describe('boundTraceId', () => {
  it.each([
    ['a plain identifier', 'trace-abc-123', 'trace-abc-123'],
    ['a dotted and colon-separated identifier', '00-4bf9.2f:1', '00-4bf9.2f:1'],
    ['surrounding whitespace', '  trace-1  ', 'trace-1'],
  ])('keeps %s', (_label, value, expected) => {
    expect(boundTraceId(value)).toBe(expected);
  });

  it.each([
    ['an absent header', undefined],
    ['an empty header', ''],
    ['a whitespace-only header', '   '],
    ['an over-long header', 'a'.repeat(129)],
    ['a header with a script fragment', '<script>alert(1)</script>'],
  ])('discards %s', (_label, value) => {
    expect(boundTraceId(value)).toBeUndefined();
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
  it('defaults to GET and upper-cases a known caller method', () => {
    expect(requestMethod()).toBe('GET');
    expect(requestMethod('post')).toBe('POST');
  });

  it.each([
    ['an unknown verb', 'break-all'],
    ['a caller-chosen label', 'GET /api/v1/users/alice@example.com'],
  ])('bounds %s to OTHER', (_label, method) => {
    expect(requestMethod(method)).toBe('OTHER');
  });
});

describe('elapsedSince', () => {
  it('never reports a negative duration', () => {
    expect(elapsedSince(performance.now() + 60_000)).toBe(0);
  });
});

describe('setFetchClock', () => {
  afterEach(() => {
    setFetchClock();
  });

  it('times operations with the installed clock', () => {
    let nowMs = 10;
    setFetchClock(() => nowMs);

    expect(fetchClockNow()).toBe(10);
    nowMs = 35;
    expect(elapsedSince(10)).toBe(25);
  });

  it('restores the platform clock when cleared', () => {
    setFetchClock(() => 0);
    setFetchClock();

    const before = fetchClockNow();
    expect(before).toBeGreaterThan(0);
    expect(elapsedSince(before)).toBeGreaterThanOrEqual(0);
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
