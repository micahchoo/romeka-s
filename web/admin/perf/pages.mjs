/**
 * The per-route payload: what a first paint downloads, and what one route adds
 * on top of it.
 *
 * `npm run build` writes `dist/.vite/manifest.json` (the `build.manifest` option
 * in `vite.config.ts`), which ties each page module to the chunk it emitted and
 * names that chunk's static imports. This script reads it, sums the gzip size of
 * a route's own chunk and the shared chunks the route pulls in, and prints the
 * table `PERF.md` quotes. Rerun it after any build to refresh those numbers.
 */
import { readFileSync } from 'node:fs';
import { basename, join } from 'node:path';

import { DIST_ROOT, measureBundle } from './bundle.mjs';

const MANIFEST_FILE = join(DIST_ROOT, '.vite', 'manifest.json');

const readManifest = () => {
  try {
    return JSON.parse(readFileSync(MANIFEST_FILE, 'utf8'));
  } catch (error) {
    throw new Error(`${MANIFEST_FILE}: run \`npm run build\` first (${String(error)})`, {
      cause: error,
    });
  }
};

/** Every measured asset by file name, so a manifest path resolves to its gzip size. */
const gzipByName = (measured) => {
  const sizes = new Map();
  for (const file of [...measured.initialFiles, ...measured.chunks]) {
    sizes.set(file.name, file.gzipBytes);
  }
  return sizes;
};

/** The shared chunks `key` imports, directly or through another shared chunk. */
const sharedImportsOf = (manifest, key, seen = new Set()) => {
  for (const imported of manifest[key]?.imports ?? []) {
    if (imported === 'index.html' || seen.has(imported)) {
      continue;
    }
    seen.add(imported);
    sharedImportsOf(manifest, imported, seen);
  }
  return seen;
};

const gzipOf = (sizes, file) => sizes.get(basename(file)) ?? 0;

const rows = (manifest, sizes) =>
  Object.keys(manifest)
    .map((key) => /^src\/pages\/([^/]+)\/page\.tsx$/.exec(key))
    .filter((match) => match !== null)
    .map((match) => {
      const key = `src/pages/${match[1]}/page.tsx`;
      const own = gzipOf(sizes, manifest[key].file);
      const shared = [...sharedImportsOf(manifest, key)].reduce(
        (total, imported) => total + gzipOf(sizes, manifest[imported].file),
        0,
      );
      return { route: match[1], own, shared, total: own + shared };
    })
    .sort((left, right) => (left.route < right.route ? -1 : 1));

const main = () => {
  const manifest = readManifest();
  const measured = measureBundle();
  const sizes = gzipByName(measured);
  console.log(
    `initial payload  js ${String(measured.initialJsGzipBytes)} B  css ${String(measured.initialCssGzipBytes)} B`,
  );
  console.log(`${String(measured.chunks.length)} lazy chunks`);
  console.log('');
  console.log(
    `${'route'.padEnd(20)} ${'own'.padStart(7)} ${'shared'.padStart(8)} ${'total'.padStart(7)}`,
  );
  for (const row of rows(manifest, sizes)) {
    console.log(
      `${row.route.padEnd(20)} ${String(row.own).padStart(7)} ${String(row.shared).padStart(8)} ${String(row.total).padStart(7)}`,
    );
  }
};

main();
