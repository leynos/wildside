/**
 * @file Vite configuration with tokens alias for the PWA.
 */
import { fileURLToPath } from 'node:url';
import react from '@vitejs/plugin-react';
import { defineConfig, loadEnv } from 'vite';
import { designTokensPlugin } from './vite/plugins/design-tokens';

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '');
  const workspaceRoot = fileURLToPath(new URL('..', import.meta.url));
  return {
    plugins: [designTokensPlugin({ workspaceRoot }), react()],
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
