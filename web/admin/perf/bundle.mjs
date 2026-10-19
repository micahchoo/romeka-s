/**
 * The bundle half of the UI budget: split the built `dist` into the payload a
 * first paint downloads and the chunks a route adds later, gzip every emitted
 * JS and CSS asset with node's zlib, and compare against `budget.json`.
 *
 * The initial payload is what `dist/index.html` references: the entry script,
 * its modulepreload links and the stylesheet. A catalogue chunk is a locale's
 * table (`src/i18n/locales/`, `src/i18n/namespaces/<id>/locales/`), which a
 * visitor in that locale fetches before the first render of the shell or of a
 * route. Everything else under `dist/assets` is a lazy chunk,
 * fetched when its route first opens. A sum over every emitted asset would be
 * a number no user waits on, so the budget gates the initial payload and gives
 * each catalogue and each chunk its own ceiling instead.
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

/**
 * Where the non-default locales' tables live, each its own chunk: the shell
 * catalogue and the server and vocabulary tables under `src/i18n/locales/`,
 * and each message namespace's under `src/i18n/namespaces/<id>/locales/`.
 */
const CATALOGUE_SOURCE = /^src\/i18n\/(namespaces\/[^/]+\/)?locales\//;

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

/**
 * The asset file names built from a locale catalogue, read from the Vite
 * manifest, the only place a hashed file is tied to its source module.
 */
export const catalogueAssetNames = (distRoot = DIST_ROOT) => {
  const path = join(distRoot, '.vite', 'manifest.json');
  let manifest;
  try {
    manifest = JSON.parse(readFileSync(path, 'utf8'));
  } catch (error) {
    throw new Error(`${path}: run \`npm run build\` first (${String(error)})`, { cause: error });
  }
  return new Set(
    Object.values(manifest)
      .filter((entry) => typeof entry.src === 'string' && CATALOGUE_SOURCE.test(entry.src))
      .map((entry) => basename(entry.file)),
  );
};

const sumGzip = (files, kind) =>
  files.filter((file) => file.kind === kind).reduce((total, file) => total + file.gzipBytes, 0);

/**
 * Gzip every `.js` and `.css` file under `dist/assets` and divide them into the
 * initial payload, the catalogue chunks and the lazy chunks. Gzipping each
 * file on its own matches what the browser downloads, one response per file.
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
  const catalogueNames = catalogueAssetNames(distRoot);
  const lazy = files.filter((file) => !initialNames.has(basename(file.name)));
  return {
    initialJsGzipBytes: sumGzip(initial, 'js'),
    initialCssGzipBytes: sumGzip(initial, 'css'),
    initialFiles: initial,
    catalogueChunks: lazy.filter((file) => catalogueNames.has(file.name)),
    chunks: lazy.filter((file) => !catalogueNames.has(file.name)),
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
  const ceilings = [
    ['chunkGzipBytes', 'chunk', measured.chunks],
    ['catalogueChunkGzipBytes', 'catalogue', measured.catalogueChunks],
  ];
  for (const [key, label, chunks] of ceilings) {
    const limit = bundle?.[key];
    if (typeof limit !== 'number') {
      failures.push(`budget.json has no bundle.${key}`);
      continue;
    }
    for (const chunk of chunks) {
      if (chunk.gzipBytes > limit) {
        failures.push(over(`${chunk.kind} ${label} ${chunk.name}`, chunk.gzipBytes, limit));
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
  for (const file of [...measured.initialFiles, ...measured.catalogueChunks, ...measured.chunks]) {
    console.log(`${file.name}  ${String(file.bytes)} bytes  gzip ${String(file.gzipBytes)}`);
  }
  console.log(
    `initial js gzip ${String(measured.initialJsGzipBytes)} bytes, initial css gzip ${String(measured.initialCssGzipBytes)} bytes, ${String(measured.catalogueChunks.length)} catalogue chunks, ${String(measured.chunks.length)} lazy chunks`,
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
