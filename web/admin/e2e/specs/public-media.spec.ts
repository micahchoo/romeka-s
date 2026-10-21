/**
 * A recording's media page reads its captions as a transcript: one language at
 * a time, every line a link to its moment, and, with a script, a "Follow along"
 * toggle that keeps the playing line in view. With no script the line's link
 * reloads the page at that moment, which the loader turns into a media
 * fragment.
 *
 * The fixture recording is half a second long, so the cues sit inside it.
 */
import { mkdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { APIRequestContext } from '@playwright/test';

import { expect, expectClean, test } from '../fixtures';
import { isArray, numberField, record } from '../harness/json';
import { SITE_SLUG } from '../harness/seed';
import { REPO_ROOT, readHarness } from '../harness/state';

const corpus = (name: string): Buffer =>
  readFileSync(join(REPO_ROOT, 'fixtures', 'media-kinds', name));

/** A WebVTT timestamp for a moment in seconds. */
const stamp = (seconds: number): string => {
  const whole = Math.floor(seconds);
  const fraction = Math.round((seconds - whole) * 1000);
  return `00:00:${String(whole).padStart(2, '0')}.${String(fraction).padStart(3, '0')}`;
};

/** Forty cues, so the transcript box scrolls and the first line reads at 0:00.
 * The second cue starts at 0.2 s, inside the half-second fixture. */
const EN_CUES = `WEBVTT

${Array.from({ length: 40 }, (_, index) => {
  const start = index * 0.2;
  return `${stamp(start)} --> ${stamp(start + 0.15)}\n${
    index === 0 ? 'A red square, then a blue bar.' : `Line number ${String(index)} speaks.`
  }`;
}).join('\n\n')}
`;

const FR_CUES = `WEBVTT

00:00:00.000 --> 00:00:00.200
Un carre rouge, puis une barre bleue.
`;

/** Upload one corpus file as the only media of a new public item. */
const upload = async (
  request: APIRequestContext,
  name: string,
  mediaType: string,
): Promise<number> => {
  const answer = await request.post('/api/items', {
    multipart: {
      data: JSON.stringify({
        'o:is_public': true,
        'dcterms:title': [{ type: 'literal', '@value': name }],
        'o:media': [{ 'o:ingester': 'upload', file_index: 0, 'o:is_public': true }],
      }),
      file: { name, mimeType: mediaType, buffer: corpus(name) },
    },
  });
  expect(answer.status(), await answer.text()).toBe(200);
  const created: unknown = await answer.json();
  const item = numberField(record(created, 'item'), 'o:id', 'item');
  const listed: unknown = await (await request.get(`/api/media?item_id=${String(item)}`)).json();
  if (!isArray(listed) || listed.length === 0) {
    throw new Error(`item ${String(item)} has no media`);
  }
  return numberField(record(listed[0], 'media'), 'o:id', 'media');
};

/** One caption import, through the route the admin calls. The request carries
 * the harness key: the proxy keys only `/api`, and this spec writes without a
 * browser session. */
const caption = async (
  request: APIRequestContext,
  media: number,
  lang: string,
  body: string,
): Promise<void> => {
  const { keyIdentity, keyCredential } = readHarness();
  const answer = await request.post(
    `/annotations/media/${String(media)}/tracks?lang=${lang}&key_identity=${keyIdentity}&key_credential=${keyCredential}`,
    { headers: { 'content-type': 'text/vtt' }, data: body },
  );
  expect(answer.status(), await answer.text()).toBe(200);
};

const captioned = async (
  request: APIRequestContext,
  name: string,
  mediaType: string,
  tracks: readonly (readonly [string, string])[],
): Promise<number> => {
  const media = await upload(request, name, mediaType);
  for (const [lang, body] of tracks) {
    await caption(request, media, lang, body);
  }
  return media;
};

const mediaPage = (media: number): string => `/s/${SITE_SLUG}/media/${String(media)}`;

test('the media page lists the captions as a transcript, each line a link to its moment', async ({
  page,
  diagnostics,
}) => {
  const media = await captioned(page.request, 'video.mp4', 'video/mp4', [
    ['en', EN_CUES],
    ['fr', FR_CUES],
  ]);
  await page.goto(mediaPage(media));

  const transcript = page.locator('section.transcript');
  await expect(transcript).toHaveCount(1);
  await expect(transcript.locator('.transcript-heading')).toHaveText(
    'Transcript · English · 40 lines',
  );
  await expect(transcript.locator('.transcript-facts')).toHaveText(
    'Captions: English, Français. Transcript below.',
  );
  const lines = transcript.locator('.transcript-line');
  await expect(lines).toHaveCount(40);
  await expect(lines.nth(0)).toContainText('A red square, then a blue bar.');
  await expect(lines.nth(1)).toContainText('Line number 1 speaks.');
  // The time reads as a reader says it, and the link states the moment the
  // loader turns into a media fragment.
  await expect(lines.nth(1).locator('.transcript-time')).toHaveText('0:00');
  expect(await lines.nth(1).locator('a').getAttribute('href')).toBe(
    `${mediaPage(media)}?t=0.2&transcript=en#player`,
  );
  // A video's player draws its own captions, so the page draws no band.
  await expect(page.locator('.transcript-band')).toHaveCount(0);

  // The other language is one link away, and the page then speaks for it.
  await transcript.getByRole('link', { name: 'Français (1)' }).click();
  await expect(page).toHaveURL(/\?transcript=fr#transcript$/);
  await expect(transcript.locator('.transcript-heading')).toHaveText(
    'Transcript · Français · 1 line',
  );
  await expect(lines.nth(0)).toContainText('Un carre rouge, puis une barre bleue.');
  await expect(transcript.getByRole('link', { name: 'Français (1)' })).toHaveAttribute(
    'aria-current',
    'true',
  );

  if (process.env.TRANSCRIPT_EVIDENCE === '1') {
    const evidence = (name: string): string => join(REPO_ROOT, '.evidence', name);
    mkdirSync(join(REPO_ROOT, '.evidence'), { recursive: true });
    await page.goto(mediaPage(media));
    await page.setViewportSize({ width: 1280, height: 900 });
    await page.screenshot({ path: evidence('transcript-1280.png'), fullPage: true });
    await page.setViewportSize({ width: 375, height: 812 });
    await page.screenshot({ path: evidence('transcript-375.png'), fullPage: true });
  }

  await expectClean(diagnostics, 'transcript');
});

test('with a script, a line seeks the player, marks itself current and follow-along scrolls its box', async ({
  page,
  diagnostics,
}) => {
  const media = await captioned(page.request, 'video.mp4', 'video/mp4', [['en', EN_CUES]]);
  await page.goto(mediaPage(media));

  const video = page.locator('.media-render video').first();
  await expect(video).toBeVisible();
  // Slow playback so the assertion has the whole moment to read.
  await video.evaluate((node: HTMLVideoElement) => {
    node.playbackRate = 0.25;
  });

  const follow = page.locator('.transcript-follow input');
  await expect(follow).toBeVisible();
  await expect(follow).toBeChecked();

  const second = page.locator('.transcript-line').nth(1);
  await second.locator('a').click();
  await expect(second).toHaveClass(/is-playing/);
  await expect(second).toHaveAttribute('aria-current', 'true');
  await expect
    .poll(async () => video.evaluate((node: HTMLVideoElement) => node.currentTime))
    .toBeGreaterThan(0.15);
  const at = await video.evaluate((node: HTMLVideoElement) => node.currentTime);
  expect(Math.abs(at - 0.2)).toBeLessThanOrEqual(0.25);
  // The address names the same moment, so a copied URL cues the player.
  await expect(page).toHaveURL(/[?&]t=0\.2(&|$)/);

  // A scroll inside the box is the reader taking over: the toggle shows it.
  const box = page.locator('.transcript-lines');
  expect(await box.evaluate((node) => node.scrollHeight > node.clientHeight)).toBe(true);
  await box.hover();
  await page.mouse.wheel(0, 200);
  await expect(follow).not.toBeChecked();

  await expectClean(diagnostics, 'transcript script');
});

test.describe('without a script', () => {
  test.use({ javaScriptEnabled: false });

  test('a line reloads the page at its moment and the player opens there', async ({ page }) => {
    const media = await captioned(page.request, 'video.mp4', 'video/mp4', [['en', EN_CUES]]);
    await page.goto(mediaPage(media));

    // The toggle is a script's control, so a reader without one never sees it.
    await expect(page.locator('.transcript-follow')).toBeHidden();
    await page.locator('.transcript-line').nth(1).locator('a').click();
    await expect(page).toHaveURL(
      new RegExp(`${mediaPage(media)}\\?t=0\\.2&transcript=en#player$`),
    );
    await expect(page.locator('.media-render video').first()).toHaveAttribute('src', /#t=0\.2$/);
  });
});

test('an audio page draws the playing line in a band, because its player draws no captions', async ({
  page,
  diagnostics,
}) => {
  const media = await captioned(page.request, 'audio.mp3', 'audio/mpeg', [['en', EN_CUES]]);
  await page.goto(mediaPage(media));

  const transcript = page.locator('section.transcript');
  await expect(transcript.locator('.transcript-facts')).toHaveText(
    'Captions show in the transcript below.',
  );
  const band = page.locator('.transcript-band');
  await expect(band).toBeHidden();

  await page.locator('.transcript-line').nth(0).locator('a').click();
  await expect(band).toBeVisible();
  await expect(band).toHaveText('A red square, then a blue bar.');

  if (process.env.TRANSCRIPT_EVIDENCE === '1') {
    mkdirSync(join(REPO_ROOT, '.evidence'), { recursive: true });
    await page.setViewportSize({ width: 1280, height: 900 });
    await page.screenshot({
      path: join(REPO_ROOT, '.evidence', 'transcript-audio-band-1280.png'),
      fullPage: true,
    });
  }

  await expectClean(diagnostics, 'transcript audio band');
});

test('the media page saves the original under its own name', async ({ page, diagnostics }) => {
  const media = await upload(page.request, 'doc.pdf', 'application/pdf');
  await page.goto(mediaPage(media));

  const link = page.locator('.media-download a');
  await expect(link).toHaveText('Download the original · PDF · 853 B');
  const download = page.waitForEvent('download');
  await link.click();
  const saved = await download;
  expect(saved.suggestedFilename()).toBe('doc.pdf');
  // A download aborts the navigation the click started; that abort is the
  // browser's normal answer, not a failed request.
  diagnostics.reset();
  await expectClean(diagnostics, 'download');
});

test('Copy puts the manifest URL on the clipboard and says so', async ({ page, diagnostics }) => {
  const media = await upload(page.request, 'image.jpg', 'image/jpeg');
  await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.goto(mediaPage(media));

  const block = page.locator('section.use-elsewhere');
  await expect(block.getByRole('heading', { name: 'Use elsewhere' })).toBeVisible();
  const manifest = await block.locator('.iiif-manifest').getAttribute('href');
  expect(manifest).not.toBeNull();
  const bare = String(manifest).split('?manifest=')[0] ?? '';
  expect(bare).not.toBe('');
  expect(String(manifest)).toBe(`${bare}?manifest=${bare}`);
  // A tool given the page URL finds the manifest in the head.
  await expect(page.locator('head link[rel="alternate"]')).toHaveAttribute('href', bare);

  await block.locator('.copy-manifest').click();
  await expect(block.locator('.copy-manifest')).toHaveText('Copied');
  await expect(block.locator('.copy-status')).toHaveText('Manifest link copied');
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(bare);
  await expectClean(diagnostics, 'copy');
});

test('a kind with no canvas offers no viewer link', async ({ page, diagnostics }) => {
  const media = await upload(
    page.request,
    'doc.docx',
    'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
  );
  await page.goto(mediaPage(media));

  const block = page.locator('section.use-elsewhere');
  await expect(block).toHaveCount(1);
  await expect(block.locator('.use-elsewhere-viewer')).toHaveCount(0);
  await expect(block.locator('.iiif-manifest')).toHaveCount(0);
  await expect(page.locator('head link[rel="alternate"]')).toHaveCount(0);
  // The file still saves under its own name; only the viewer and manifest are
  // absent, because there is no canvas to open.
  await expect(page.locator('.media-download a')).toHaveCount(1);
  // An office document is a document, not a file with no kind.
  await expect(page.locator('.media-download')).toContainText('Document');
  await expectClean(diagnostics, 'a kind with no canvas');
});

test('a narrow window shows the PDF card instead of the browser reader', async ({
  page,
  diagnostics,
}) => {
  const media = await upload(page.request, 'doc.pdf', 'application/pdf');
  await page.setViewportSize({ width: 375, height: 812 });
  await page.goto(mediaPage(media));

  // The reader is an 80vh box a narrow column cannot use, so the page shows the
  // card and the download line. A hidden reader must leave no gap behind it.
  await expect(page.locator('.pdf-object')).toBeHidden();
  const card = page.locator('.pdf-reader-page > .pdf-card');
  await expect(card).toBeVisible();

  const cardBox = await card.boundingBox();
  const downloadBox = await page.locator('.media-download').boundingBox();
  if (cardBox === null || downloadBox === null) {
    throw new Error('the card and the download line have no box at 375 px');
  }
  const gap = downloadBox.y - (cardBox.y + cardBox.height);
  expect(gap).toBeLessThan(40);
  await expectClean(diagnostics, 'narrow pdf');
});
