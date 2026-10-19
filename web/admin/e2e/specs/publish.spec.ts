/**
 * Publishing end to end: a term list is published from its page, its scheme
 * and each term answer at `/vocab/<slug>/` in Turtle and as a page, a save
 * that drops a published word is refused with the plain message, a deprecated
 * word reaches the next version, a merge on a published list keeps the merged
 * word as a deprecated term the item editor no longer offers, and the local
 * vocabulary "Create a new field" makes publishes at its own namespace.
 */
import { expect, expectClean, test } from '../fixtures';
import { isArray, numberField, record, stringField } from '../harness/json';

const LIST = 'E2E published places';
const SLUG = 'e2e-places';

test('a term list publishes, serves its terms and keeps them', async ({ page, diagnostics }) => {
  const created = await page.request.post('/api/custom_vocabs', {
    data: { 'o:label': LIST, 'o:lang': 'en', 'o:terms': 'Cork\nDublin\nGalway' },
  });
  expect(created.status(), await created.text()).toBe(200);
  const listId = numberField(record(await created.json(), 'term list'), 'o:id', 'term list');

  await page.goto(`/#/term-lists/${String(listId)}`);
  const panel = page.getByRole('region', { name: 'Publish' });
  await expect(panel.getByText('Draft. Not published yet.')).toBeVisible();
  await panel.getByLabel('Address name').fill(SLUG);
  await expect(panel.getByText(`/vocab/${SLUG}/`, { exact: false }).first()).toBeVisible();
  await panel.getByRole('button', { name: 'Publish', exact: true }).click();
  await expect(panel.getByText('Published version 1.')).toBeVisible();
  await expect(panel.getByText(/^Published\. Version 1, .+\.$/)).toBeVisible();
  await expectClean(diagnostics, 'first publish');

  const turtle = await page.request.get(`/vocab/${SLUG}/`, { headers: { accept: 'text/turtle' } });
  expect(turtle.status()).toBe(200);
  expect(turtle.headers()['content-type']).toBe('text/turtle; charset=utf-8');
  const origin = new URL(page.url()).origin;
  const body = await turtle.text();
  expect(body).toContain(`<${origin}/vocab/${SLUG}/> a skos:ConceptScheme`);
  expect(body).toContain(`<${origin}/vocab/${SLUG}/2> a skos:Concept`);

  const jsonLd = await page.request.get(`/vocab/${SLUG}/3`, {
    headers: { accept: 'application/ld+json' },
  });
  const nodes: unknown = await jsonLd.json();
  expect(isArray(nodes) ? nodes.map((node) => stringField(record(node, 'node'), '@id', 'node')) : []).toEqual([
    `${origin}/vocab/${SLUG}/3`,
  ]);

  // A browser asks for HTML and gets the scheme's page.
  await page.goto(`/vocab/${SLUG}/`);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText(LIST);
  await page.getByRole('link', { name: 'Dublin' }).click();
  await expect(page).toHaveURL(new RegExp(`/vocab/${SLUG}/2$`));
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Dublin');
  await expectClean(diagnostics, 'public pages');

  // Dropping a published word is refused, and the form says why.
  await page.goto(`/#/term-lists/${String(listId)}`);
  await page.getByLabel('Words, one per line').fill('Cork\nDublin');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText(
    'A published term cannot be removed or renamed: "Galway". Deprecate it instead.',
  );
  const stored = record(await (await page.request.get(`/api/custom_vocabs/${String(listId)}`)).json(), 'list');
  expect(stored['o:terms']).toEqual(['Cork', 'Dublin', 'Galway']);
  await diagnostics.idle();
  expect(diagnostics.failedRequests).toEqual([
    expect.stringMatching(new RegExp(`^422 .*/api/custom_vocabs/${String(listId)}\\b`)),
  ]);
  diagnostics.reset();

  // Deprecating it instead reaches the next version.
  await page.reload();
  const reloaded = page.getByRole('region', { name: 'Publish' });
  await expect(reloaded.getByText('Nothing changed since version 1.')).toBeVisible();
  await reloaded.getByRole('button', { name: 'Deprecate Galway' }).click();
  await expect(reloaded.getByText('Deprecated from the next version')).toBeVisible();
  await reloaded.getByRole('button', { name: 'Publish version 2' }).click();
  await expect(reloaded.getByText('Published version 2.')).toBeVisible();
  await expect(reloaded.getByRole('cell', { name: 'Deprecated', exact: true })).toBeVisible();
  await expect(reloaded.getByRole('button', { name: /^Publish/ })).toHaveCount(0);
  await expectClean(diagnostics, 'deprecate and republish');

  const galway = await (
    await page.request.get(`/vocab/${SLUG}/3`, { headers: { accept: 'text/turtle' } })
  ).text();
  expect(galway).toContain('owl:deprecated true');
  const first = await (
    await page.request.get(`/vocab/${SLUG}/3?version=1`, { headers: { accept: 'text/turtle' } })
  ).text();
  expect(first).not.toContain('owl:deprecated');
});

test('a merge on a published list keeps the merged word, deprecated and replaced', async ({
  page,
  diagnostics,
}) => {
  const created = await page.request.post('/api/custom_vocabs', {
    data: { 'o:label': 'E2E merged towns', 'o:lang': 'en', 'o:terms': 'Cork\nCo. Cork\nBantry' },
  });
  const listId = numberField(record(await created.json(), 'term list'), 'o:id', 'term list');
  const published = await page.request.post(`/api/custom_vocabs/${String(listId)}/publication`, {
    data: { action: 'publish', slug: 'e2e-towns' },
  });
  expect(published.status(), await published.text()).toBe(200);
  const spatial: unknown = await (await page.request.get('/api/properties?term=dcterms:spatial')).json();
  const spatialId = numberField(record(isArray(spatial) ? spatial[0] : null, 'spatial'), 'o:id', 'spatial');
  const item = await page.request.post('/api/items', {
    data: {
      'dcterms:title': [{ type: 'literal', property_id: 'auto', '@value': 'E2E town map' }],
      'dcterms:spatial': [
        { type: `customvocab:${String(listId)}`, property_id: spatialId, '@value': 'Co. Cork' },
      ],
    },
  });
  const itemId = numberField(record(await item.json(), 'item'), 'o:id', 'item');

  await page.goto(`/#/term-lists/${String(listId)}`);
  const prompt = page.getByRole('region', { name: 'Similar spellings' });
  await prompt.getByRole('radio', { name: /Keep “Cork”/ }).check();
  await prompt.getByRole('button', { name: 'Merge into “Cork”' }).click();
  await expect(prompt).toContainText('“Co. Cork” will stay as deprecated, replaced by “Cork”');
  await prompt.getByRole('button', { name: 'Merge', exact: true }).click();
  await expect(page.getByText('Merged into “Cork”. Resources changed: 1.')).toBeVisible();
  await expect(prompt).toHaveCount(0);
  const panel = page.getByRole('region', { name: 'Publish' });
  const row = panel.getByRole('row').filter({ hasText: 'Co. Cork' });
  await expect(row).toContainText('Deprecated from the next version');
  await expect(row).toContainText('Replaced by “Cork”');
  await panel.getByRole('button', { name: 'Publish version 2' }).click();
  await expect(panel.getByText('Published version 2.')).toBeVisible();
  await expectClean(diagnostics, 'merge on a published list');

  const term = await (
    await page.request.get('/vocab/e2e-towns/2', { headers: { accept: 'text/turtle' } })
  ).text();
  const origin = new URL(page.url()).origin;
  expect(term).toContain('owl:deprecated true');
  expect(term).toContain(`dcterms:isReplacedBy <${origin}/vocab/e2e-towns/1>`);

  // The item editor no longer offers the deprecated word.
  await page.goto(`/#/items/${String(itemId)}`);
  await page.getByRole('button', { name: 'Edit this item' }).click();
  const picker = page.getByRole('combobox', { name: /Term list: E2E merged towns/ });
  await expect(picker).toHaveValue('Cork');
  await picker.fill('co');
  await expect(page.locator('.term-picker').getByRole('option')).toHaveText(['Cork']);
  await expectClean(diagnostics, 'picker after merge');
});

test('the local vocabulary publishes at its own namespace', async ({ page, diagnostics }) => {
  const origin = new URL((await page.goto('/'))?.url() ?? '').origin;
  const vocabularies: unknown = await (await page.request.get('/api/vocabularies?prefix=local')).json();
  const existing = isArray(vocabularies) ? vocabularies[0] : undefined;
  const vocabularyId =
    existing === undefined
      ? numberField(
          record(
            await (
              await page.request.post('/api/vocabularies', {
                data: {
                  'o:namespace_uri': `${origin}/vocab/local/`,
                  'o:prefix': 'local',
                  'o:label': 'Local fields',
                  'o:comment': 'Fields this archive made for itself.',
                },
              })
            ).json(),
            'vocabulary',
          ),
          'o:id',
          'vocabulary',
        )
      : numberField(record(existing, 'vocabulary'), 'o:id', 'vocabulary');
  const property = await page.request.post('/api/properties', {
    data: {
      'o:local_name': 'e2eShelfMark',
      'o:label': 'Shelf mark',
      'o:vocabulary': { 'o:id': vocabularyId },
    },
  });
  expect(property.status(), await property.text()).toBe(200);

  await page.goto(`/#/standards/vocabularies/${String(vocabularyId)}`);
  const panel = page.getByRole('region', { name: 'Publish' });
  await expect(panel.getByText(`${origin}/vocab/local/e2eShelfMark`, { exact: true })).toBeVisible();
  await panel.getByRole('button', { name: /^Publish/ }).click();
  await expect(panel.getByText(/^Published version \d+\.$/)).toBeVisible();
  await expectClean(diagnostics, 'vocabulary publish');

  const term = await page.request.get('/vocab/local/e2eShelfMark', {
    headers: { accept: 'text/turtle' },
  });
  expect(term.status()).toBe(200);
  expect(await term.text()).toContain(`<${origin}/vocab/local/e2eShelfMark> a rdf:Property`);
});
