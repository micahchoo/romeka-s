/// <reference types="node" />
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

// The running `romeka serve` the dev server forwards to.
const devTarget = process.env.ROMEKA_DEV_TARGET ?? 'http://localhost:8080';

export default defineConfig({
  plugins: [react()],
  // The Rust server mounts the built SPA under an arbitrary path, so assets
  // must be referenced relatively.
  base: './',
  server: {
    // The port's own routes, forwarded so the dev origin is the one origin the
    // app, the API, the files, the public site and its preview see, the way
    // `deploy/Caddyfile` arranges it in production. A preview is a document
    // written into an iframe on this origin, so its theme sheet under `/assets`
    // must resolve here too; the dev server serves no `/assets` of its own.
    proxy: Object.fromEntries(
      ['/api', '/api-context', '/s', '/files', '/assets'].map((path) => [path, devTarget]),
    ),
  },
  build: {
    // `perf/pages.mjs` reads this to name the chunk each route adds. The
    // manifest is the only place a hashed chunk is tied to its source module.
    manifest: true,
    rolldownOptions: {
      output: {
        // The Theme page is three pieces that load in parallel, each under the
        // per-chunk ceiling (`perf/budget.json`, PERF.md Decision 10): its
        // messages in two languages, its palette arithmetic and registry, and
        // the page itself.
        codeSplitting: {
          // A group takes only the modules its test names; React and the
          // shared app modules stay where the default splitting puts them.
          includeDependenciesRecursively: false,
          groups: [
            { name: 'theme-messages', test: /pages\/sites\/themeEditor\/messages\.ts$/ },
            {
              name: 'theme-model',
              test: /(pages\/sites\/themeEditor\/(colour|palette|registry|draft)\.ts|generated\/themeSettings\.json)$/,
            },
          ],
        },
      },
    },
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./tests/setup.ts'],
    // The Playwright specs under e2e/ match Vitest's default `*.spec.ts` glob.
    include: ['tests/**/*.test.{ts,tsx}'],
  },
});
