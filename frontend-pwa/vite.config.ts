/**
 * @file Vite configuration with tokens alias for the PWA.
 */
import { defineConfig, loadEnv } from 'vite';
import { resolve } from 'node:path';
import react from '@vitejs/plugin-react';

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '');
  return {
    resolve: {
      alias: {
        '@app/tokens': resolve(__dirname, '../packages/tokens/dist'),
      },
    },
    plugins: [react()],
    server: {
      host: true,
      port: 5173,
      strictPort: true,
      proxy: {
        '/api': {
          target: 'http://localhost:8080',
          changeOrigin: true,
          ws: true,
        },
      },
    },
    build: { sourcemap: env.SOURCEMAP === 'true' },
  };
});
