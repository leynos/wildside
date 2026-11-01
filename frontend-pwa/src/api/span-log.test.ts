/** @file Bounded retention contracts for the client span log. */
import { afterEach, describe, expect, it } from 'bun:test';

import { createSpanLog, publishSpanLog } from './span-log';
import type { ClientSpan } from './tracing';

/** A distinguishable span for the given route template. */
const spanOf = (name: string): ClientSpan => ({
  name,
  method: 'GET',
  startedAt: 0,
  durationMs: 1,
  status: 'ok',
});

afterEach(() => {
  globalThis.__wildsideClientSpans = undefined;
});

describe('createSpanLog', () => {
  it('retains spans in order, oldest first', () => {
    const log = createSpanLog();

    log.sink(spanOf('/api/v1/users'));
    log.sink(spanOf('/api/v1/routes/:id'));

    expect(log.snapshot().spans.map((span) => span.name)).toEqual([
      '/api/v1/users',
      '/api/v1/routes/:id',
    ]);
  });

  it('discards the oldest span once the bound is reached', () => {
    const log = createSpanLog(2);

    log.sink(spanOf('/api/v1/users'));
    log.sink(spanOf('/api/v1/routes'));
    log.sink(spanOf('/api/v1/routes/:id'));

    expect(log.snapshot().spans.map((span) => span.name)).toEqual([
      '/api/v1/routes',
      '/api/v1/routes/:id',
    ]);
  });

  it('returns a copy so a reader cannot edit retained spans', () => {
    const log = createSpanLog();
    log.sink(spanOf('/api/v1/users'));

    const first = log.snapshot();
    // SAFETY: the snapshot is a fresh array, so mutating it cannot reach the
    // log's own buffer. The public type is `readonly` to stop ordinary callers.
    (first.spans as ClientSpan[]).length = 0;

    expect(log.snapshot().spans).toHaveLength(1);
  });

  it('publishes a snapshot accessor for devtools', () => {
    const log = createSpanLog();
    log.sink(spanOf('/api/v1/users'));

    publishSpanLog(log);

    expect(globalThis.__wildsideClientSpans?.().spans).toHaveLength(1);
  });
});
