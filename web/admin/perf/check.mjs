/**
 * The UI budget gate. Exits non-zero when the initial payload, a lazy chunk or a
 * recorded timing is over its budget in `budget.json`.
 *
 * The bundle is measured from `dist` on every run, so the gate catches a bundle
 * regression without an e2e run. The timings come from `perf/summary.json`,
 * which the `e2e/specs/budget.spec.ts` run writes; when that file is absent the
 * gate says so and checks the bundle alone.
 *
 * Run with `npm run perf:check` after `npm run build && npm run e2e`.
 */
import { existsSync, readFileSync } from 'node:fs';

import { APP_ROOT, SUMMARY_FILE, bundleFailures, measureBundle, readBudget } from './bundle.mjs';

/** The timing keys of the budget and of the summary, in report order. */
const TIMING_KEYS = ['fcp', 'listRender', 'detailRender', 'createRoundTrip', 'navigation'];

/** `summary.timings[key].median`, or `null` when the summary does not carry it. */
const medianOf = (timings, key) => {
  const value = timings?.[key]?.median;
  return typeof value === 'number' ? value : null;
};

const timingFailures = (budget, summary) => {
  const timings = summary?.timings;
  return TIMING_KEYS.flatMap((key) => {
    const limit = budget?.timings?.[key];
    if (typeof limit !== 'number') {
      return [`budget.json has no timings.${key}`];
    }
    const median = medianOf(timings, key);
    if (median === null) {
      return [`summary.timings.${key}.median is missing`];
    }
    return median > limit
      ? [`${key} median ${median.toFixed(1)} ms is over the ${String(limit)} ms budget`]
      : [];
  });
};

const row = (label, actual, limit, unit) => {
  const cell = actual === null ? 'n/a' : unit === 'B' ? String(Math.round(actual)) : actual.toFixed(1);
  console.log(
    `${label.padEnd(17)} ${cell.padStart(9)} ${unit.padEnd(4)} budget ${String(limit).padStart(6)}`,
  );
};

const main = () => {
  const budget = readBudget();
  const measured = measureBundle();
  const failures = bundleFailures(budget, measured);
  const largest = measured.chunks.reduce(
    (best, chunk) => (best === null || chunk.gzipBytes > best.gzipBytes ? chunk : best),
    null,
  );
  const largestCatalogue = Math.max(0, ...measured.catalogueChunks.map((chunk) => chunk.gzipBytes));
  console.log(`dist measured from ${APP_ROOT}/dist`);
  row('initial js', measured.initialJsGzipBytes, budget.bundle.initialJsGzipBytes, 'B');
  row('initial css', measured.initialCssGzipBytes, budget.bundle.initialCssGzipBytes, 'B');
  row('largest catalogue', largestCatalogue, budget.bundle.catalogueChunkGzipBytes, 'B');
  row('largest chunk', largest === null ? null : largest.gzipBytes, budget.bundle.chunkGzipBytes, 'B');
  console.log(
    `lazy chunks: ${String(measured.chunks.length)}, largest ${largest === null ? 'none' : largest.name}`,
  );

  const summary = existsSync(SUMMARY_FILE)
    ? JSON.parse(readFileSync(SUMMARY_FILE, 'utf8'))
    : null;
  if (summary === null) {
    console.log(`timings: ${SUMMARY_FILE} is absent, run \`npm run e2e\` to record them; skipped`);
  } else {
    console.log(
      `timings from ${SUMMARY_FILE} (${String(summary.runs)} runs, commit ${String(summary.commit)})`,
    );
    for (const key of TIMING_KEYS) {
      row(key, medianOf(summary.timings, key), budget.timings[key], 'ms');
    }
    failures.push(...timingFailures(budget, summary));
  }

  if (failures.length === 0) {
    console.log('ui budget: ok');
    process.exitCode = 0;
    return;
  }
  for (const failure of failures) {
    console.error(`budget: ${failure}`);
  }
  console.error(`ui budget: ${String(failures.length)} exceeded`);
  process.exitCode = 1;
};

main();
