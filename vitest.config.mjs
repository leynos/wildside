/** @file Root Vitest configuration for repository script tests. */

import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'node',
    include: ['scripts/**/*.test.mjs'],
  },
});
