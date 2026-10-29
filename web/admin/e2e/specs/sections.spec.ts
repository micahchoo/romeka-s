/**
 * Sections end to end: a new page gets the "Media and text" section from the
 * block picker, the curator writes its empty text block, it saves, and the
 * public page lays its two blocks side by side on a desktop and one under the
 * other on a phone. The picker offers the bare block group as "Empty
 * section", and the narrow Outline stacks a section's slots.
 */
import { expect, test } from '../fixtures';
import type { Locator } from '@playwright/test';
import { SITE_SLUG, SITE_TITLE } from '../harness/seed';

const SLUG = 'media-and-text';

/** The left edge and top of a block on the public page. */
const corner = async (block: Locator): Promise<{ readonly x: number; readonly y: number }> => {
  const box = await block.boundingBox();
  if (box === null) {
    throw new Error('the block has no box');
  }
  return { x: box.x, y: box.y };
};

test('the Media and text section is two columns on a desktop and one on a phone', async ({
  page,
}) => {
  await page.goto('/#/sites');
  await page.getByRole('link', { name: SITE_TITLE }).click();
  await page.getByRole('link', { name: 'Add page' }).click();
  await page.getByRole('button', { name: 'Start blank' }).click();

  await page.getByRole('button', { name: /Insert block at position/ }).last().click();
  const picker = page.getByRole('dialog', { name: 'Insert a block or template' });
  await picker.getByRole('button', { name: /^Media and text/ }).click();

  // The Outline draws the section as its two slots, and the inspector shows
  // the Section panel with the 40/60 diagram chosen.
  const section = page.getByRole('listitem', { name: 'Section, Narrow and wide' });
  await expect(section.locator('.canvas-slot')).toHaveCount(2);
  await expect(page.getByRole('radio', { name: 'Narrow and wide' })).toHaveAttribute(
    'aria-checked',
    'true',
  );

  // The section's text block starts empty, with its hint; the curator writes it.
  await section.locator('.canvas-slot').nth(1).getByText('Text', { exact: true }).click();
  const words = page.getByRole('textbox', { name: 'Text', exact: true });
  await expect(words).toHaveAttribute('data-placeholder', 'Write what visitors read here.');
  await words.fill('A line for this part');

  // The page's own settings come back when the selection is cleared.
  await page.getByRole('button', { name: 'Page', exact: true }).click();
  await page.getByLabel('Title').fill('Media and text');
  await page.getByLabel('Slug').fill(SLUG);
  await page.getByRole('button', { name: 'Add page' }).click();
  await expect(page).toHaveURL(/#\/sites\/1\/pages\/\d+/);

  await page.setViewportSize({ width: 1280, height: 900 });
  const response = await page.goto(`/s/${SITE_SLUG}/page/${SLUG}`);
  expect(response?.status()).toBe(200);
  const group = page.locator('.romeka-section.romeka-section-split-40-60');
  await expect(group).toHaveCount(1);
  const media = group.locator('> .block-media');
  const text = group.locator('> .block-html');
  await expect(text.getByText('A line for this part')).toBeVisible();

  const wide = { media: await corner(media), text: await corner(text) };
  expect(wide.text.x).toBeGreaterThan(wide.media.x + 200);
  expect(Math.abs(wide.text.y - wide.media.y)).toBeLessThan(200);

  await page.setViewportSize({ width: 390, height: 844 });
  const narrow = { media: await corner(media), text: await corner(text) };
  expect(narrow.text.x).toBe(narrow.media.x);
  expect(narrow.text.y).toBeGreaterThanOrEqual(narrow.media.y);
  // One column fills the width the page column gives it.
  const columns = await group.evaluate((element) => getComputedStyle(element).gridTemplateColumns);
  expect(columns.trim().split(/\s+/)).toHaveLength(1);
});

test('the picker offers the bare group as Empty section, and the Outline stacks a section of cards', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto('/#/sites');
  await page.getByRole('link', { name: SITE_TITLE }).click();
  await page.getByRole('link', { name: 'Add page' }).click();
  await page.getByRole('button', { name: 'Start blank' }).click();

  await page.getByRole('button', { name: /Insert block at position/ }).last().click();
  const picker = page.getByRole('dialog', { name: 'Insert a block or template' });
  await expect(picker.getByRole('button', { name: 'Block group', exact: true })).toHaveCount(0);
  await expect(picker.getByRole('button', { name: /^Empty section/ })).toBeVisible();
  await picker.getByRole('button', { name: /^Three cards/ }).click();

  // Beside the preview the Outline is too narrow for three columns of titles,
  // so the slots stack: one left edge, each below the one before.
  const slots = page.locator('.canvas-section .canvas-slot');
  await expect(slots).toHaveCount(3);
  for (const width of [1440, 1280]) {
    await page.setViewportSize({ width, height: 900 });
    const boxes = await slots.evaluateAll((elements) =>
      elements.map((element) => {
        const box = element.getBoundingClientRect();
        return { x: box.x, y: box.y };
      }),
    );
    const [first, second, third] = boxes;
    if (first === undefined || second === undefined || third === undefined) {
      throw new Error('three slots have no boxes');
    }
    expect(second.x).toBe(first.x);
    expect(third.x).toBe(first.x);
    expect(second.y).toBeGreaterThan(first.y);
    expect(third.y).toBeGreaterThan(second.y);
  }

  await page.getByRole('button', { name: /Insert block at position/ }).last().click();
  await picker.getByRole('button', { name: /^Empty section/ }).click();
  await expect(page.getByRole('listitem', { name: /: Block group$/ })).toHaveCount(1);
  await expect(page.locator('.canvas-group:not(.canvas-section)')).toContainText('0 blocks');
});
