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
        mutator: {
          path: 'src/api/fetcher.ts',
          // Use the named export to avoid generating a default import.
          name: 'customFetch',
          default: false,
        },
      },
    },
  },
};

module.exports = config;
