/**
 * @file Vite configuration with tokens alias for the PWA.
 */
import { execSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import react from '@vitejs/plugin-react';
import { defineConfig, loadEnv } from 'vite';

function ensureTokensDist(workspaceRoot: string) {
  const tokensRoot = resolve(workspaceRoot, 'packages/tokens');
  const tokensDistPath = resolve(tokensRoot, 'dist');
  if (existsSync(tokensDistPath)) return tokensDistPath;

  const buildCommand = 'pnpm --filter @app/tokens build';
  try {
    execSync(buildCommand, {
      cwd: workspaceRoot,
      stdio: 'inherit',
    });
  } catch (error) {
    const guidance = [
      'Failed to build design tokens automatically.',
      `Tried running \`${buildCommand}\`.`,
      'Fix the error above or build the tokens manually.',
    ].join(' ');
    throw new Error(guidance, { cause: error as Error });
  }

  if (!existsSync(tokensDistPath)) {
    throw new Error(
      'Design tokens build output missing after running the build command.',
    );
  }

  return tokensDistPath;
}

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '');
  const projectRoot = fileURLToPath(new URL('.', import.meta.url));
  const workspaceRoot = fileURLToPath(new URL('..', import.meta.url));
  const tokensDistPath = ensureTokensDist(workspaceRoot);
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
