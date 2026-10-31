/** @file Contract checks for the Orval generation configuration. */

import { describe, expect, it } from 'vitest';

import config from '../frontend-pwa/orval.config.cjs';

const { override } = config.frontendClient.output;

describe('orval configuration', () => {
  it('uses the validated fetcher adapter as its mutator', () => {
    expect(override.mutator).toMatchObject({
      path: 'src/api/fetcher.ts',
      name: 'customFetchOrval',
      default: false,
    });
  });

  it('generates bare response bodies rather than HTTP envelopes', () => {
    // `customFetchOrval` resolves to the validated body, so generated callers
    // that expect `data` and `status` fields would read `undefined` at runtime.
    expect(override.fetch).toEqual({ includeHttpResponseReturnType: false });
  });
});