/**
 * The site check on the hub, against the real server, on a site of its own.
 *
 * A site made through the API starts with the starter Welcome page, whose
 * introduction is left for the curator to write. A second public page holds a
 * featured item that is private. The check names both, and each line's links
 * open the page that fixes it.
 */
import type { APIRequestContext } from '@playwright/test';

import { expect, expectClean, test } from '../fixtures';
import { readHarness } from '../harness/state';

const idOf = async (answer: Awaited<ReturnType<APIRequestContext['post']>>): Promise<number> => {
  expect(answer.status(), await answer.text()).toBe(200);
  const document: unknown = await answer.json();
  if (typeof document === 'object' && document !== null && 'o:id' in document && typeof document['o:id'] === 'number') {
    return document['o:id'];
  }
  throw new Error(`no id in ${JSON.stringify(document)}`);
};

test('the site check counts an unwritten block and a private featured item, and links each page', async ({
  page,
  request,
  diagnostics,
}) => {
  const site = await idOf(
    await request.post('/api/sites', {
      data: { 'o:title': 'E2E check site', 'o:slug': 'e2e-check', 'o:theme': 'default', 'o:is_public': true },
    }),
  );
  const item = await idOf(await request.post('/api/items', { data: { 'o:is_public': false } }));
  const shelf = await idOf(
    await request.post('/api/site_pages', {
      data: {
        'o:site': { 'o:id': site },
        'o:title': 'Shelf',
        'o:slug': 'shelf',
        'o:is_public': true,
        'o:block': [
          {
            'o:layout': 'html',
            'o:data': { html: '' },
            'o:layout_data': { romeka: { block: 'featuredItem', v: 1, settings: { item } } },
          },
        ],
      },
    }),
  );

  // The page editor's preview POSTs to `/s/<slug>/preview`, which the harness
  // proxy does not key, so the browser holds the key as a signed-in user's does.
  const { keyIdentity, keyCredential } = readHarness();
  await page.addInitScript(
    (key: { identity: string; credential: string }) => {
      window.localStorage.setItem('romeka.apiKey', JSON.stringify(key));
    },
    { identity: keyIdentity, credential: keyCredential },
  );
  await page.goto(`/#/sites/${String(site)}`);
  await page.getByRole('button', { name: 'Check this site' }).click();
  const check = page.getByRole('region', { name: 'Before you share this site' });
  const unwritten = check.getByRole('listitem').filter({ hasText: '1 block not written, on 1 public page' });
  const hidden = check
    .getByRole('listitem')
    .filter({ hasText: '1 block points to private items or media, on 1 public page' });
  await expect(unwritten).toBeVisible();
  await expect(hidden).toBeVisible();
  await expect(check.getByRole('link', { name: '1 public page is not in the menu' })).toHaveAttribute(
    'href',
    `#/sites/${String(site)}/navigation`,
  );

  await hidden.getByRole('link', { name: 'Shelf' }).click();
  await expect(page).toHaveURL(new RegExp(`#/sites/${String(site)}/pages/${String(shelf)}$`));
  // A fresh load of the hub starts the check unopened again.
  await page.goto(`/#/sites/${String(site)}`);
  await page.reload();
  await page.getByRole('button', { name: 'Check this site' }).click();
  await unwritten.getByRole('link', { name: 'Welcome' }).click();
  await expect(page).toHaveURL(new RegExp(`#/sites/${String(site)}/pages/\\d+$`));
  await expect(page.getByRole('navigation', { name: 'Breadcrumb' }).getByText('Welcome')).toHaveAttribute(
    'aria-current',
    'page',
  );
  await expectClean(diagnostics, 'site check');
});

test('on a private site the check says so first, and counts a media list of a private item', async ({
  page,
  request,
  diagnostics,
}) => {
  const site = await idOf(
    await request.post('/api/sites', {
      data: { 'o:title': 'E2E private check', 'o:slug': 'e2e-check-private', 'o:theme': 'default', 'o:is_public': false },
    }),
  );
  // A private item on the site, holding one public media: the public site lists the media, but a
  // visitor can open neither its file nor its item.
  await idOf(
    await request.post('/api/items', {
      data: {
        'o:is_public': false,
        'o:site': [{ 'o:id': site }],
        'o:media': [{ 'o:ingester': 'html', html: '<p>A note</p>', 'o:is_public': true }],
      },
    }),
  );
  await idOf(
    await request.post('/api/site_pages', {
      data: {
        'o:site': { 'o:id': site },
        'o:title': 'Notes',
        'o:slug': 'notes',
        'o:is_public': true,
        'o:block': [{ 'o:layout': 'browsePreview', 'o:data': { resource_type: 'media', query: '', limit: 12 } }],
      },
    }),
  );

  await page.goto(`/#/sites/${String(site)}`);
  await page.getByRole('button', { name: 'Check this site' }).click();
  const check = page.getByRole('region', { name: 'Before you share this site' });
  const first = check.getByRole('listitem').first();
  await expect(first).toContainText('This site is private: visitors see none of it.');
  await expect(first.getByRole('link', { name: 'Site settings' })).toHaveAttribute(
    'href',
    `#/sites/${String(site)}/settings`,
  );
  await expect(check.getByText('Once the site is public, visitors meet these:')).toBeVisible();
  const hidden = check
    .getByRole('listitem')
    .filter({ hasText: '1 block points to private items or media, on 1 public page' });
  await expect(hidden.getByRole('link', { name: 'Notes' })).toBeVisible();
  // The media list shows a visitor one media, so it is not among the lists that show nothing.
  await expect(
    check.getByRole('listitem').filter({ hasText: /shows? nothing to visitors/ }).getByRole('link', { name: 'Notes' }),
  ).toHaveCount(0);
  await expectClean(diagnostics, 'site check on a private site');
});

test('the check counts blocks still showing template text as the editor marks them', async ({
  page,
  request,
  diagnostics,
}) => {
  const site = await idOf(
    await request.post('/api/sites', {
      data: { 'o:title': 'E2E template check', 'o:slug': 'e2e-check-template', 'o:theme': 'default', 'o:is_public': true },
    }),
  );
  const hero = await idOf(
    await request.post('/api/site_pages', {
      data: {
        'o:site': { 'o:id': site },
        'o:title': 'Hero',
        'o:slug': 'hero',
        'o:is_public': true,
        'o:block': [
          {
            'o:layout': 'html',
            'o:data': { html: '' },
            'o:layout_data': {
              romeka: {
                block: 'hero',
                v: 2,
                settings: { heading: 'Your exhibit title', subheading: 'A sentence about what this exhibit holds' },
              },
            },
          },
          { 'o:layout': 'html', 'o:data': { html: '<p>A line worth remembering.</p>' } },
          { 'o:layout': 'html', 'o:data': { html: '<p>Our own words.</p>' } },
        ],
      },
    }),
  );

  const { keyIdentity, keyCredential } = readHarness();
  await page.addInitScript(
    (key: { identity: string; credential: string }) => {
      window.localStorage.setItem('romeka.apiKey', JSON.stringify(key));
    },
    { identity: keyIdentity, credential: keyCredential },
  );
  await page.goto(`/#/sites/${String(site)}`);
  await page.getByRole('button', { name: 'Check this site' }).click();
  const check = page.getByRole('region', { name: 'Before you share this site' });
  const template = check
    .getByRole('listitem')
    .filter({ hasText: '2 blocks still show template text, on 1 public page' });
  await expect(template).toContainText(
    'Visitors read this text as written. Clear it or write over it in the page editor.',
  );

  await template.getByRole('link', { name: 'Hero' }).click();
  await expect(page).toHaveURL(new RegExp(`#/sites/${String(site)}/pages/${String(hero)}$`));
  await expect(page.getByText('Template text', { exact: true })).toHaveCount(2);
  await expectClean(diagnostics, 'site check of template text');
});
