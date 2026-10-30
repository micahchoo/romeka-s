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
      ['/api', '/api-context', '/logout', '^/s(/|$)', '/files', '/assets', '/vocab', '/iiif', '/annotations'].map((path) => [path, devTarget]),
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
            // A message namespace's English table is its own chunk, shared by
            // the routes that read it (`src/i18n/namespaces/`).
            // A delegate goes with its table: the registries it guards parse on
            // load, and a page chunk between them would close an import cycle.
            {
              name: 'theme-messages',
              test: /(i18n\/namespaces\/theme\/index|pages\/sites\/themeEditor\/messages)\.ts$/,
            },
            // The page editor's field messages in English (PERF.md Decision 14).
            {
              name: 'editor-messages',
              test: /(i18n\/namespaces\/editor\/index|pages\/sites\/editor\/messages)\.ts$/,
            },
            { name: 'common-messages', test: /i18n\/namespaces\/common\/index\.ts$/ },
            { name: 'resources-messages', test: /i18n\/namespaces\/resources\/index\.ts$/ },
            { name: 'sites-messages', test: /i18n\/namespaces\/sites\/index\.ts$/ },
            { name: 'admin-messages', test: /i18n\/namespaces\/admin\/index\.ts$/ },
            { name: 'account-messages', test: /i18n\/namespaces\/account\/index\.ts$/ },
            { name: 'standards-messages', test: /i18n\/namespaces\/standards\/index\.ts$/ },
            { name: 'item-types-messages', test: /i18n\/namespaces\/item-types\/index\.ts$/ },
            { name: 'term-lists-messages', test: /i18n\/namespaces\/term-lists\/index\.ts$/ },
            // The notes panel's table, which the panel, the region editor and the
            // time editor all read, so no one page chunk carries it for the others.
            { name: 'annotations-messages', test: /i18n\/namespaces\/annotations\/index\.ts$/ },
            // The add-media panel's table, so the panel chunk holds only the panel.
            { name: 'media-add-messages', test: /i18n\/namespaces\/media-add\/index\.ts$/ },
            // The lessons, which several page chunks share (PERF.md Decision 16).
            { name: 'lessons', test: /(app\/lessons\/(lesson\.tsx|messages\.ts|queue\.ts|registry\.ts|seen\.ts)|i18n\/namespaces\/lessons\/index\.ts)$/ },
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
    // The persistent transform cache keys a module by its own text, but an
    // `import.meta.glob` transform also depends on which files match
    // (`tests/i18n-sync.test.ts` globs every locale file): a new or removed
    // file leaves the cached list stale and the test fails on files that are
    // fine. Every run transforms from scratch instead.
    fsModuleCache: false,
  },
});
