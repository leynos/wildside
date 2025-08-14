/**
 * @file Vite configuration with tokens alias for the PWA.
 */
import { defineConfig } from 'vite';
import { resolve } from 'path';
import react from '@vitejs/plugin-react';

export default defineConfig({
  resolve: {
    alias: {
      '@app/tokens': resolve(__dirname, '../packages/tokens/dist')
    }
  },
  plugins: [react()],
  server: { port: 5173, strictPort: true },
  build: { sourcemap: true }
});
