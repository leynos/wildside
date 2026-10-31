/**
 * @file Orval configuration used to generate the typed frontend API client.
 * Aligns with the backend OpenAPI dump and reuses the custom fetch mutator.
 */
/** @type {import('orval').Config} */
const config = {
  frontendClient: {
    input: '../spec/openapi.json',
    output: {
      target: 'src/api/generated/client.ts',
      client: 'fetch',
      override: {
        // `customFetchOrval` resolves to the validated body, not an HTTP
        // envelope, so generated operations must return the bare response
        // type. Otherwise callers would read `data`/`status` off a body that
        // never carries them.
        fetch: {
          includeHttpResponseReturnType: false,
        },
        mutator: {
          path: 'src/api/fetcher.ts',
          // Use the named export to avoid generating a default import.
          // `customFetchOrval` accepts the response type argument Orval emits,
          // while delegating validation to the strict `customFetch` helper.
          name: 'customFetchOrval',
          default: false,
        },
      },
    },
  },
};

module.exports = config;
