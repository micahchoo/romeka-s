/**
 * The end-to-end suite for the admin SPA.
 *
 * `globalSetup` builds the app, installs a temporary database, starts the port
 * and serves `dist` with the API forwarded, so these specs drive the real app
 * against the real server in a real browser. Run them with `npm run e2e` from
 * `web/admin`, or `npm run e2e -- specs/items.spec.ts` for one file.
 */
import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './specs',
  globalSetup: './global-setup.ts',
  // Every spec writes to the one seeded database through the one server.
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 30_000,
  outputDir: './test-results',
  reporter: [['list']],
  use: {
    ...devices['Desktop Chrome'],
    // Traces are off by default. Recording one for every test wrote files that
    // were still being closed when the run ended, which failed a spec with
    // `browserContext.close: ENOENT` and made the gate flaky. Set PW_TRACE=1 to
    // record them while debugging a failure locally.
    trace: process.env.PW_TRACE ? 'retain-on-failure' : 'off',
    screenshot: 'only-on-failure',
  },
});
