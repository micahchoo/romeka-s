import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [react()],
  // The Rust server mounts the built SPA under an arbitrary path, so assets
  // must be referenced relatively.
  base: './',
  build: {
    // `perf/pages.mjs` reads this to name the chunk each route adds. The
    // manifest is the only place a hashed chunk is tied to its source module.
    manifest: true,
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./tests/setup.ts'],
    // The Playwright specs under e2e/ match Vitest's default `*.spec.ts` glob.
    include: ['tests/**/*.test.{ts,tsx}'],
  },
});
