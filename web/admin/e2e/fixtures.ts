/**
 * The fixtures every spec starts from.
 *
 * `baseURL` comes from the record `globalSetup` wrote, so a spec navigates with
 * `page.goto('/#/items')` and reaches the ephemeral port the run bound.
 * `diagnostics` collects what the browser reported while the spec drove the app,
 * which is what the page-load spec asserts on.
 */
import { expect, test as base } from '@playwright/test';
import type { ConsoleMessage, Request, Response } from '@playwright/test';

import { readHarness } from './harness/state';

/** How long the network has to stay quiet before a step counts as finished. */
const QUIET_MS = 300;
const POLL_MS = 50;
const IDLE_TIMEOUT_MS = 5_000;

/**
 * The session probe's URL, `GET /api/me`.
 *
 * An anonymous caller gets 401 there by contract, and a browser logs every 4xx
 * response as a console error, so the probe's own answer is neither a failed
 * request nor a page error. Chromium locates that console message at the request
 * URL, so this names it without hiding anything a script logged. Every other
 * status on this route, and every status on any other route, still counts.
 */
const isSessionProbe = (url: string): boolean => {
  try {
    return new URL(url).pathname === '/api/me';
  } catch {
    return false;
  }
};

const isSessionProbeAnswer = (response: Response): boolean =>
  response.status() === 401 &&
  response.request().method() === 'GET' &&
  isSessionProbe(response.url());

export interface Diagnostics {
  /** Console errors since the last reset. */
  readonly consoleErrors: string[];
  /** Failed requests and responses at or above 400 since the last reset. */
  readonly failedRequests: string[];
  /** Forget what happened so far, so the next step is attributed on its own. */
  readonly reset: () => void;
  /** Wait until the page has no request in flight, so a late failure is seen. */
  readonly idle: () => Promise<void>;
}

interface Fixtures {
  readonly baseURL: string;
  readonly diagnostics: Diagnostics;
}

export const test = base.extend<Fixtures>({
  baseURL: async ({}, use) => {
    await use(readHarness().baseUrl);
  },
  diagnostics: async ({ page }, use) => {
    const consoleErrors: string[] = [];
    const failedRequests: string[] = [];
    const pending = new Map<Request, string>();
    let lastChange = Date.now();
    const onConsole = (message: ConsoleMessage): void => {
      if (message.type() === 'error' && !isSessionProbe(message.location().url)) {
        consoleErrors.push(message.text());
      }
    };
    const onRequest = (request: Request): void => {
      pending.set(request, request.url());
      lastChange = Date.now();
    };
    const onRequestDone = (request: Request): void => {
      pending.delete(request);
      lastChange = Date.now();
    };
    const onRequestFailed = (request: Request): void => {
      failedRequests.push(
        `${request.method()} ${request.url()} ${request.failure()?.errorText ?? 'failed'}`,
      );
    };
    const onResponse = (response: Response): void => {
      if (response.status() >= 400 && !isSessionProbeAnswer(response)) {
        failedRequests.push(`${String(response.status())} ${response.url()}`);
      }
    };
    page.on('console', onConsole);
    page.on('request', onRequest);
    page.on('requestfinished', onRequestDone);
    page.on('requestfailed', onRequestDone);
    page.on('requestfailed', onRequestFailed);
    page.on('response', onResponse);
    await use({
      consoleErrors,
      failedRequests,
      reset: () => {
        consoleErrors.length = 0;
        failedRequests.length = 0;
      },
      idle: async () => {
        const deadline = Date.now() + IDLE_TIMEOUT_MS;
        while (Date.now() < deadline) {
          if (pending.size === 0 && Date.now() - lastChange >= QUIET_MS) {
            return;
          }
          await new Promise<void>((done) => {
            setTimeout(done, POLL_MS);
          });
        }
        // A request still in flight is not a failure the browser reported, so
        // name it here rather than letting the step pass as clean.
        for (const url of pending.values()) {
          failedRequests.push(`still in flight after ${String(IDLE_TIMEOUT_MS)}ms: ${url}`);
        }
      },
    });
  },
});

/**
 * Assert the browser reported nothing for the step just taken. `label` names the
 * step in the failure, so a broken page is named by the entry that broke it.
 */
export const expectClean = async (diagnostics: Diagnostics, label: string): Promise<void> => {
  await diagnostics.idle();
  expect(diagnostics.consoleErrors, `${label}: console errors`).toEqual([]);
  expect(diagnostics.failedRequests, `${label}: failed requests`).toEqual([]);
};

export { expect };
