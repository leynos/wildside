/**
 * @file Orval configuration used to generate the typed frontend API client.
 * Aligns with the backend OpenAPI dump and reuses the custom fetch mutator.
 */
module.exports = {
  frontendClient: {
    input: '../spec/openapi.json',
    output: {
      target: 'src/api/generated/client.ts',
      client: 'fetch',
      override: {
        mutator: {
          path: 'src/api/fetcher.ts',
          name: 'customFetch',
          default: false,
        },
      },
    },
  },
};
