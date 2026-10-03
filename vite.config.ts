import { fileURLToPath } from 'node:url';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { defineConfig } from 'vitest/config';

const repoRoot = fileURLToPath(new URL('.', import.meta.url));

export default defineConfig({
  root: 'ui',
  plugins: [svelte()],
  // Tauri reads the dev server on a fixed port and prints Rust errors itself.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // The interface imports the logo from assets/brand, outside ui/.
    fs: { allow: [repoRoot] },
  },
  build: {
    outDir: '../dist',
    emptyOutDir: true,
    target: 'es2022',
  },
  resolve: {
    // Makes Svelte resolve to its browser build under Vitest.
    conditions: process.env.VITEST ? ['browser'] : [],
  },
  test: {
    environment: 'jsdom',
  },
});
