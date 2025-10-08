/**
 * @file Vite configuration with tokens alias for the PWA.
 */
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import react from '@vitejs/plugin-react';
import { defineConfig, loadEnv } from 'vite';

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '');
  const projectRoot = fileURLToPath(new URL('.', import.meta.url));
  const tokensDistPath = resolve(projectRoot, '../packages/tokens/dist');
  if (!existsSync(tokensDistPath)) {
    const warningMessage = [
      'Design tokens build output not found.',
      'The pre-scripts should have built it automatically.',
      'If this persists, run `pnpm --filter @app/tokens build` manually.',
    ].join(' ');
    process.stderr.write(`${warningMessage}\n`);
  }
  return {
    resolve: {
      alias: {
        '@app/tokens': tokensDistPath,
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
