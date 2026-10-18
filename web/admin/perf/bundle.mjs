/**
 * The bundle half of the UI budget: split the built `dist` into the payload a
 * first paint downloads and the chunks a route adds later, gzip every emitted
 * JS and CSS asset with node's zlib, and compare against `budget.json`.
 *
 * The initial payload is what `dist/index.html` references: the entry script,
 * its modulepreload links and the stylesheet. Everything else under
 * `dist/assets` is a lazy chunk, fetched when its route first opens. A sum over
 * every emitted asset would be a number no user waits on, so the budget gates
 * the initial payload and gives each chunk its own ceiling instead.
 *
 * `npm run perf:bundle` runs it over the tree's own `dist` and exits non-zero on
 * an exceeded budget. `npm run perf:check` imports the same functions, so the
 * number the spec records and the number the gate enforces come from one place.
 * A `--json` flag prints the measurement alone, which is how the Playwright spec
 * folds it into `summary.json`.
 */
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { basename, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { gzipSync } from 'node:zlib';

export const PERF_DIR = fileURLToPath(new URL('.', import.meta.url));
export const APP_ROOT = resolve(PERF_DIR, '..');
export const DIST_ROOT = join(APP_ROOT, 'dist');
export const BUDGET_FILE = join(PERF_DIR, 'budget.json');
export const SUMMARY_FILE = join(PERF_DIR, 'summary.json');

/** Parse `budget.json`. */
export const readBudget = (path = BUDGET_FILE) => JSON.parse(readFileSync(path, 'utf8'));

/** `js` or `css` from the extension, or `null` for an asset the budget ignores. */
const kindOf = (name) => {
  if (name.endsWith('.js')) {
    return 'js';
  }
  return name.endsWith('.css') ? 'css' : null;
};

/** One measured asset, without the directory it lives in. */
const measure = (assetsDir, name) => {
  const path = join(assetsDir, name);
  return {
    name,
    kind: kindOf(name),
    bytes: statSync(path).size,
    gzipBytes: gzipSync(readFileSync(path)).length,
  };
};

/**
 * The asset file names `index.html` references, which is the payload the first
 * paint waits on. A name that appears there counts as initial even when the
 * browser would have fetched it anyway.
 */
export const initialAssetNames = (distRoot = DIST_ROOT) => {
  const html = readFileSync(join(distRoot, 'index.html'), 'utf8');
  return new Set([...html.matchAll(/assets\/([^"'()\s]+\.(?:js|css))/g)].map((match) => match[1]));
};

const sumGzip = (files, kind) =>
  files.filter((file) => file.kind === kind).reduce((total, file) => total + file.gzipBytes, 0);

/**
 * Gzip every `.js` and `.css` file under `dist/assets` and divide them into the
 * initial payload and the lazy chunks. Gzipping each file on its own matches
 * what the browser downloads, one response per file.
 */
export const measureBundle = (distRoot = DIST_ROOT) => {
  const assetsDir = join(distRoot, 'assets');
  let names;
  try {
    names = readdirSync(assetsDir);
  } catch (error) {
    throw new Error(
      `${assetsDir}: no assets directory, run \`npm run build\` first (${String(error)})`,
      { cause: error },
    );
  }
  const files = names
    .filter((name) => kindOf(name) !== null)
    .sort()
    .map((name) => measure(assetsDir, name));
  if (files.length === 0) {
    throw new Error(`${assetsDir}: no .js or .css file to measure`);
  }
  const initialNames = initialAssetNames(distRoot);
  const initial = files.filter((file) => initialNames.has(basename(file.name)));
  if (!initial.some((file) => file.kind === 'js')) {
    throw new Error(`${join(distRoot, 'index.html')}: references no js asset`);
  }
  return {
    initialJsGzipBytes: sumGzip(initial, 'js'),
    initialCssGzipBytes: sumGzip(initial, 'css'),
    initialFiles: initial,
    chunks: files.filter((file) => !initialNames.has(basename(file.name))),
  };
};

const over = (label, actual, limit) =>
  `${label} gzip ${String(actual)} bytes is over the ${String(limit)} byte budget by ${String(actual - limit)}`;

/** The budget failures for one measurement, each a sentence naming the number. */
export const bundleFailures = (budget, measured) => {
  const failures = [];
  const bundle = budget?.bundle;
  if (typeof bundle?.initialJsGzipBytes !== 'number') {
    failures.push('budget.json has no bundle.initialJsGzipBytes');
  } else if (measured.initialJsGzipBytes > bundle.initialJsGzipBytes) {
    failures.push(over('initial js', measured.initialJsGzipBytes, bundle.initialJsGzipBytes));
  }
  if (typeof bundle?.initialCssGzipBytes !== 'number') {
    failures.push('budget.json has no bundle.initialCssGzipBytes');
  } else if (measured.initialCssGzipBytes > bundle.initialCssGzipBytes) {
    failures.push(over('initial css', measured.initialCssGzipBytes, bundle.initialCssGzipBytes));
  }
  if (typeof bundle?.chunkGzipBytes !== 'number') {
    failures.push('budget.json has no bundle.chunkGzipBytes');
  } else {
    for (const chunk of measured.chunks) {
      if (chunk.gzipBytes > bundle.chunkGzipBytes) {
        failures.push(
          over(`${chunk.kind} chunk ${chunk.name}`, chunk.gzipBytes, bundle.chunkGzipBytes),
        );
      }
    }
  }
  return failures;
};

const main = () => {
  const measured = measureBundle();
  if (process.argv.includes('--json')) {
    process.stdout.write(`${JSON.stringify(measured)}\n`);
    return;
  }
  for (const file of [...measured.initialFiles, ...measured.chunks]) {
    console.log(`${file.name}  ${String(file.bytes)} bytes  gzip ${String(file.gzipBytes)}`);
  }
  console.log(
    `initial js gzip ${String(measured.initialJsGzipBytes)} bytes, initial css gzip ${String(measured.initialCssGzipBytes)} bytes, ${String(measured.chunks.length)} lazy chunks`,
  );
  const failures = bundleFailures(readBudget(), measured);
  for (const failure of failures) {
    console.error(`budget: ${failure}`);
  }
  process.exitCode = failures.length === 0 ? 0 : 1;
};

if (resolve(process.argv[1] ?? '') === fileURLToPath(import.meta.url)) {
  main();
}
