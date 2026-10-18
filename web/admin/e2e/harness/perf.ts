/**
 * The in-browser timing probe and the `summary.json` writer for the UI budget.
 *
 * The numbers come from the Performance API inside a real browser, driven by
 * Playwright: the first contentful paint of a cold load, and the time each user
 * path takes from the click that starts it to the DOM that finishes it. The
 * page-side waits run in the page (`measureInPage` and `fcpInPage` are
 * serialized into the document), so a round trip between the worker and the
 * browser is not part of the measured span.
 *
 * `measureInPage` resolves on the DOM mutation that carries the finished view,
 * not on the next paint. It is a render-readiness number, not a paint number;
 * the first contentful paint is the paint number. Each result also carries the
 * wall span of the `/api` requests the step made, read from resource timing on
 * the same clock, so a number can be attributed to the API or to the render.
 */
import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import type { Page } from '@playwright/test';

import { isArray, numberField, parseJson, record, stringField } from './json';
import { APP_ROOT, REPO_ROOT } from './state';

/** How many samples every timing takes. The hedge is the run-to-run range. */
export const RUNS = 5;

/** What one in-page measurement needs: the click that starts it and the finished DOM. */
export interface ProbeSpec {
  /** CSS selector of the element to click. The first match is clicked. */
  readonly action: string;
  /** CSS selector of an element that only exists once the step finished. */
  readonly ready: string;
  /** How many `ready` matches count as finished. Defaults to one. */
  readonly min?: number;
  /** Text one `ready` match must contain. Omitted when the selector is enough. */
  readonly text?: string;
}

/** One measured step. */
export interface ProbeResult {
  /** Milliseconds from the click to the finished DOM. */
  readonly totalMs: number;
  /** Wall span of the `/api` requests inside the step, or `null` when the step made none. */
  readonly apiMs: number | null;
  /** The longest single `/api` request inside the step, or `null` when the step made none. */
  readonly apiMaxMs: number | null;
  /** Pathname of the longest `/api` request, or `null` when the step made none. */
  readonly apiSlowest: string | null;
  /** How many `/api` requests the step made. */
  readonly apiRequests: number;
}

export interface TimingStat {
  readonly unit: 'ms';
  readonly runs: number;
  readonly min: number;
  readonly median: number;
  readonly max: number;
  /** Median of the per-run API wall spans; `null` when the path makes no API request. */
  readonly apiMedian: number | null;
  /** Median of the longest single API request per run; `null` when the path makes no API request. */
  readonly apiMaxMedian: number | null;
  /** The endpoint that was slowest in the most runs; `null` when the path makes no API request. */
  readonly apiSlowest: string | null;
  /** How many `/api` requests one run of the path made. */
  readonly apiRequests: number;
}

export type TimingKey = 'fcp' | 'listRender' | 'detailRender' | 'createRoundTrip' | 'navigation';

export interface MeasuredFile {
  readonly name: string;
  readonly kind: string;
  readonly bytes: number;
  readonly gzipBytes: number;
}

export interface BundleMeasure {
  /** Gzip size of the assets `index.html` references, which the first paint waits on. */
  readonly initialJsGzipBytes: number;
  readonly initialCssGzipBytes: number;
  readonly initialFiles: readonly MeasuredFile[];
  /** Every other emitted js or css asset, fetched when its route first opens. */
  readonly chunks: readonly MeasuredFile[];
}

export interface Summary {
  readonly commit: string;
  readonly generatedAt: string;
  readonly runs: number;
  readonly bundle: BundleMeasure;
  readonly timings: Readonly<Record<TimingKey, TimingStat>>;
}

/** `web/admin/perf`. */
export const perfDir = (): string => join(APP_ROOT, 'perf');
/** The artifact `perf:check` reads. */
export const summaryFile = (): string => join(perfDir(), 'summary.json');

/**
 * Click `spec.action` and resolve when `spec.ready` holds, timing the span with
 * `performance.now` and reading the `/api` requests out of resource timing on
 * the same clock. Runs inside the page, so the clock covers the click, the fetch
 * and the render.
 */
export const measureInPage = async (spec: ProbeSpec): Promise<ProbeResult> => {
  const timeout = 20_000;
  const deadline = Date.now() + timeout;
  const min = spec.min ?? 1;
  const text = spec.text;
  const ready = (): boolean => {
    const nodes = document.querySelectorAll(spec.ready);
    if (nodes.length < min) {
      return false;
    }
    if (text === undefined) {
      return true;
    }
    return Array.from(nodes).some((node) => node.textContent.includes(text));
  };
  const target = document.querySelector(spec.action);
  if (!(target instanceof HTMLElement)) {
    throw new Error(`probe: no clickable element for ${spec.action}`);
  }
  // Resource timing appends one entry per finished request, so everything past
  // this index happened inside the measured span.
  const seen = performance.getEntriesByType('resource').length;
  const start = performance.now();
  target.click();
  await new Promise<void>((resolve, reject) => {
    const observer = new MutationObserver(check);
    observer.observe(document.documentElement, {
      childList: true,
      subtree: true,
      attributes: true,
      characterData: true,
    });
    const timer = setInterval(check, 16);
    function finish(): void {
      observer.disconnect();
      clearInterval(timer);
    }
    function check(): void {
      if (ready()) {
        finish();
        resolve();
        return;
      }
      if (Date.now() > deadline) {
        finish();
        reject(new Error(`probe: ${spec.ready} not ready within ${String(timeout)}ms`));
      }
    }
  });
  const totalMs = performance.now() - start;
  // Count only requests that began inside the span. A request still in flight
  // from the previous page started earlier and is not this step's cost.
  const api = performance
    .getEntriesByType('resource')
    .slice(seen)
    .filter((entry) => entry.name.includes('/api/') && entry.startTime >= start);
  if (api.length === 0) {
    return { totalMs, apiMs: null, apiMaxMs: null, apiSlowest: null, apiRequests: 0 };
  }
  const firstStart = Math.min(...api.map((entry) => entry.startTime));
  const lastEnd = Math.max(...api.map((entry) => entry.startTime + entry.duration));
  const slowest = api.reduce((left, right) => (right.duration > left.duration ? right : left));
  return {
    totalMs,
    apiMs: lastEnd - firstStart,
    apiMaxMs: slowest.duration,
    apiSlowest: new URL(slowest.name).pathname,
    apiRequests: api.length,
  };
};

/** The browser's first-contentful-paint start time, or `null` when it never fired. */
export const fcpInPage = async (): Promise<number | null> => {
  const deadline = Date.now() + 2_000;
  for (;;) {
    const entry = performance
      .getEntriesByType('paint')
      .find((candidate) => candidate.name === 'first-contentful-paint');
    if (entry !== undefined) {
      return entry.startTime;
    }
    if (Date.now() > deadline) {
      return null;
    }
    await new Promise<void>((done) => {
      setTimeout(done, 10);
    });
  }
};

/** The paint timing of `runs` cold loads. Cache is disabled, so each one re-fetches the bundle. */
export const measureFcp = async (page: Page, baseURL: string, runs: number): Promise<number[]> => {
  const session = await page.context().newCDPSession(page);
  await session.send('Network.setCacheDisabled', { cacheDisabled: true });
  const samples: number[] = [];
  try {
    for (let index = 0; index < runs; index += 1) {
      await page.goto(`${baseURL}/#/items`, { waitUntil: 'load' });
      const start = await page.evaluate(fcpInPage);
      if (start === null) {
        throw new Error('no first-contentful-paint entry after the load event');
      }
      samples.push(start);
    }
  } finally {
    await session.send('Network.setCacheDisabled', { cacheDisabled: false });
    await session.detach();
  }
  return samples;
};

/** Run `step` `runs` times, calling `reset` before each so every sample starts from the same state. */
export const repeat = async (
  runs: number,
  reset: () => Promise<void>,
  step: (index: number) => Promise<ProbeResult>,
): Promise<ProbeResult[]> => {
  const results: ProbeResult[] = [];
  for (let index = 0; index < runs; index += 1) {
    await reset();
    results.push(await step(index));
  }
  return results;
};

const median = (values: readonly number[]): number => {
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1
    ? (sorted[middle] ?? 0)
    : ((sorted[middle - 1] ?? 0) + (sorted[middle] ?? 0)) / 2;
};

const present = (value: number | null): value is number => value !== null;

/** The value that appears most often, or `null` for an empty list. */
const mode = (values: readonly string[]): string | null => {
  const counts = new Map<string, number>();
  for (const value of values) {
    counts.set(value, (counts.get(value) ?? 0) + 1);
  }
  let best: string | null = null;
  let bestCount = 0;
  for (const [value, count] of counts) {
    if (count > bestCount) {
      best = value;
      bestCount = count;
    }
  }
  return best;
};

const stat = (
  samples: readonly number[],
  apiSpans: readonly number[],
  apiMaxes: readonly number[],
  apiSlowest: readonly string[],
): TimingStat => {
  if (samples.length === 0) {
    throw new Error('summarize: no samples');
  }
  return {
    unit: 'ms',
    runs: samples.length,
    min: Math.min(...samples),
    median: median(samples),
    max: Math.max(...samples),
    apiMedian: apiSpans.length === 0 ? null : median(apiSpans),
    apiMaxMedian: apiMaxes.length === 0 ? null : median(apiMaxes),
    apiSlowest: mode(apiSlowest),
    apiRequests: 0,
  };
};

/** Summarize a path measured with `measureInPage`, keeping the API wall span. */
export const summarizeProbes = (results: readonly ProbeResult[]): TimingStat => {
  const summarized = stat(
    results.map((result) => result.totalMs),
    results.map((result) => result.apiMs).filter(present),
    results.map((result) => result.apiMaxMs).filter(present),
    results.map((result) => result.apiSlowest).filter((value): value is string => value !== null),
  );
  const apiRequests = median(results.map((result) => result.apiRequests));
  return { ...summarized, apiRequests: Math.round(apiRequests) };
};

/** Summarize a path that carries no API attribution, such as the paint timing. */
export const summarize = (samples: readonly number[]): TimingStat => stat(samples, [], [], []);

/**
 * Measure `dist` through `perf/bundle.mjs --json`. One implementation of the
 * gzip rule serves both the spec and `perf:check`.
 */
export const measureBundleFromDist = (): BundleMeasure => {
  const stdout = execFileSync('node', [join(perfDir(), 'bundle.mjs'), '--json'], {
    cwd: APP_ROOT,
    encoding: 'utf8',
  });
  const doc = record(parseJson(stdout), 'bundle');
  const files = (value: unknown, field: string): MeasuredFile[] => {
    if (!isArray(value)) {
      return [];
    }
    return value.map((entry, index) => {
      const path = `bundle.${field}[${String(index)}]`;
      const row = record(entry, path);
      return {
        name: stringField(row, 'name', path),
        kind: stringField(row, 'kind', path),
        bytes: numberField(row, 'bytes', path),
        gzipBytes: numberField(row, 'gzipBytes', path),
      };
    });
  };
  return {
    initialJsGzipBytes: numberField(doc, 'initialJsGzipBytes', 'bundle'),
    initialCssGzipBytes: numberField(doc, 'initialCssGzipBytes', 'bundle'),
    initialFiles: files(doc.initialFiles, 'initialFiles'),
    chunks: files(doc.chunks, 'chunks'),
  };
};

export const headCommit = (): string =>
  execFileSync('git', ['rev-parse', 'HEAD'], { cwd: REPO_ROOT, encoding: 'utf8' }).trim();

/** Write `perf/summary.json`, the artifact `perf:check` reads. */
export const writeSummary = (
  timings: Readonly<Record<TimingKey, TimingStat>>,
  bundle: BundleMeasure,
): Summary => {
  const summary: Summary = {
    commit: headCommit(),
    generatedAt: new Date().toISOString(),
    runs: RUNS,
    bundle,
    timings,
  };
  mkdirSync(perfDir(), { recursive: true });
  writeFileSync(summaryFile(), `${JSON.stringify(summary, null, 2)}\n`);
  return summary;
};
