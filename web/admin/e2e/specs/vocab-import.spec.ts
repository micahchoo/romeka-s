/**
 * Advanced > Standards > Vocabularies: import a published vocabulary from a
 * file, then update it from a newer copy, reviewing the diff before it is
 * applied. The file is upstream's own SKOS copy under the importer's fixtures.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { expect, expectClean, test } from '../fixtures';
import { expectAxeBudget } from '../harness/axe';
import { REPO_ROOT } from '../harness/state';

const SKOS_FILE = join(REPO_ROOT, 'crates/omeka-api/tests/fixtures/rdf/skos.rdf');
const SKOS = 'http://www.w3.org/2004/02/skos/core#';

const skos = (): Buffer => readFileSync(SKOS_FILE);

test('a vocabulary file is imported, then updated from a newer copy after review', async ({
  page,
  diagnostics,
}) => {
  await page.goto('/#/standards');
  await page.getByRole('link', { name: 'Import a vocabulary' }).click();
  await expect(page.getByRole('heading', { name: 'Import a vocabulary' })).toBeVisible();

  const help = page.getByRole('button', { name: 'What Namespace URI means' });
  await help.click();
  await expect(page.getByText('Only the terms under it are imported.')).toBeVisible();

  await page.getByLabel('Vocabulary file').setInputFiles({
    name: 'skos.rdf',
    mimeType: 'application/rdf+xml',
    buffer: skos(),
  });
  await page.getByRole('textbox', { name: 'Namespace URI' }).fill(SKOS);
  await page.getByRole('textbox', { name: 'Prefix' }).fill('skos');
  await page.getByRole('textbox', { name: 'Label', exact: true }).fill('SKOS');
  await expectAxeBudget(page, 'vocabulary import form');
  await page.getByRole('button', { name: 'Import vocabulary' }).click();

  await expect(page).toHaveURL(/#\/standards\/vocabularies\/\d+$/);
  await expect(page.getByRole('heading', { name: 'SKOS', level: 2 })).toBeVisible();
  await expect(page.getByRole('link', { name: 'preferred label' })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Concept Scheme' })).toBeVisible();
  await expectClean(diagnostics, 'skos imported');

  // A newer copy renames one property and drops one class.
  const newer = skos()
    .toString('utf8')
    .replace('<rdfs:label xml:lang="en">preferred label</rdfs:label>', '<rdfs:label xml:lang="en">preferred name</rdfs:label>')
    .replace(/<rdf:Description rdf:about="#OrderedCollection">[\s\S]*?<\/rdf:Description>/, '');
  await page.getByRole('button', { name: 'Update from a file or web address' }).click();
  await page.getByLabel('Vocabulary file').setInputFiles({
    name: 'skos-newer.rdf',
    mimeType: 'application/rdf+xml',
    buffer: Buffer.from(newer),
  });
  await page.getByRole('button', { name: 'Compare' }).click();

  await expect(page.getByText('Review these changes before you apply them.')).toBeVisible();
  const changed = page.getByRole('heading', { name: 'Changed labels and comments (1)' });
  await expect(changed).toBeVisible();
  await expect(page.getByRole('cell', { name: 'preferred name' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'No longer in the file (1)' })).toBeVisible();
  await expect(page.getByText('Ordered Collection (skos:OrderedCollection)')).toBeVisible();
  await expectAxeBudget(page, 'vocabulary diff');

  await page.getByRole('button', { name: 'Apply these changes' }).click();
  await expect(page.getByText('The changes are saved.')).toBeVisible();
  await expect(page.getByRole('link', { name: 'preferred name' })).toBeVisible();
  // The dropped class is kept.
  await expect(page.getByRole('link', { name: 'Ordered Collection' })).toBeVisible();
  await expectClean(diagnostics, 'skos updated');

  // Written, not only drawn.
  await page.reload();
  await expect(page.getByRole('link', { name: 'preferred name' })).toBeVisible();
});

test('a file with nothing in the namespace is refused with the reason', async ({ page }) => {
  await page.goto('/#/standards/vocabularies/import');
  await page.getByLabel('Vocabulary file').setInputFiles({
    name: 'skos.rdf',
    mimeType: 'application/rdf+xml',
    buffer: skos(),
  });
  await page.getByRole('textbox', { name: 'Namespace URI' }).fill('http://example.org/not-skos#');
  await page.getByRole('textbox', { name: 'Prefix' }).fill('notskos');
  await page.getByRole('textbox', { name: 'Label', exact: true }).fill('Not SKOS');
  await page.getByRole('button', { name: 'Import vocabulary' }).click();
  await expect(
    page.getByText(
      'No classes or properties found. Are you sure you used the correct namespace URI?',
    ),
  ).toBeVisible();
  await expect(page).toHaveURL(/#\/standards\/vocabularies\/import$/);
});
