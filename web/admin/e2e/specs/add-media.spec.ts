/**
 * Adding media from the item page: a file with a description, a video that has
 * no preview, a YouTube link, and a researcher who may read the media but not
 * add one.
 *
 * The files are `fixtures/media-kinds/`, the same corpus the matrix reads, so
 * the kinds are real bytes rather than names the panel guesses.
 */
import { expect, expectClean, test } from '../fixtures';
import { resolve } from 'node:path';
import { ensureCli, runCli } from '../harness/cli';
import { readHarness } from '../harness/state';

const MEDIA = resolve(import.meta.dirname, '../../../../fixtures/media-kinds');
const TITLE = 'E2E item with added media';
const ALT = 'A hall at dusk, the door open';

test.describe.configure({ mode: 'serial', timeout: 180_000 });
test.use({ actionTimeout: 20_000 });

test('the item page adds an image, a video and a link, and a researcher sees no way to add', async ({
  page,
  diagnostics,
}) => {
  // A new item through the form, so the item page opens on its empty media state.
  await page.goto('/#/items');
  await page.getByRole('link', { name: 'Add item' }).click();
  await page.getByRole('textbox', { name: 'Title', exact: true }).fill(TITLE);
  await page.getByRole('button', { name: 'Add item' }).click();
  await expect(page).toHaveURL(/#\/items\/\d+$/u);
  const itemUrl = page.url();
  await expect(page.getByRole('heading', { name: 'Media (0)', level: 2 })).toBeVisible();
  await expect(page.getByText('No media yet. Drop a file here, choose one, or paste a link.')).toBeVisible();
  await expectClean(diagnostics, 'the empty item page');

  // A file: the panel opens, the bytes go to the staging area, and the row says Added.
  await page.getByRole('button', { name: 'Add media' }).click();
  const panel = page.getByRole('region', { name: `Add media to ${TITLE}` });
  await expect(panel).toBeVisible();
  await page.setInputFiles('#add-media-files', resolve(MEDIA, 'image.jpg'));
  const open = panel.getByRole('link', { name: /Open media #\d+/u });
  await expect(open).toBeVisible({ timeout: 30_000 });
  await expect(panel.getByText('Added', { exact: true })).toBeVisible();

  // The description travels with the media, and the media page is where it is read.
  const describe = panel.getByLabel('Describe the image');
  await describe.fill(ALT);
  await describe.press('Tab');
  await expect(panel.getByText('Saved.')).toBeVisible();

  const mediaUrl = (await open.getAttribute('href')) ?? '';
  const mediaId = Number(/media\/(\d+)/u.exec(mediaUrl)?.[1]);
  expect(mediaId).toBeGreaterThan(0);
  await open.click();
  await expect(page).toHaveURL(new RegExp(`#/media/${String(mediaId)}$`, 'u'));
  await expect(page.getByRole('heading', { name: 'image.jpg', level: 2 })).toBeVisible({ timeout: 30_000 });
  await expect(page.getByText(ALT)).toBeVisible();
  await expectClean(diagnostics, 'the added image and its description');

  // A video: no derivative exists yet, so the card names the kind and shows no picture.
  await page.goto(itemUrl);
  await page.getByRole('button', { name: 'Add media' }).click();
  await page.setInputFiles('#add-media-files', resolve(MEDIA, 'video.mp4'));
  await expect(page.getByRole('region', { name: `Add media to ${TITLE}` }).getByText('Added', { exact: true })).toBeVisible({ timeout: 30_000 });
  const videoCard = page.locator('.media-card').filter({ hasText: 'video.mp4' });
  await expect(videoCard).toBeVisible();
  // The server's fallback thumbnail is a picture of no media, so the kind badge carries the word.
  await expect(videoCard).toContainText('Video');
  await expect(videoCard).toContainText('video/mp4');
  await expectClean(diagnostics, 'the added video');

  // A link: the probe names what it is before anything saves, and the card is an embed.
  await page.getByLabel('or paste a link').fill('https://www.youtube.com/watch?v=dQw4w9WgXcQ');
  await expect(page.getByText(/^A YouTube video/u)).toBeVisible({ timeout: 20_000 });
  await page.getByRole('button', { name: 'Add YouTube video' }).click();
  const embedCard = page.locator('.media-card').filter({ hasText: 'youtu' });
  await expect(embedCard).toBeVisible({ timeout: 30_000 });
  await expect(embedCard).toContainText('Embed');
  await expect(page.getByRole('heading', { name: 'Media (3)', level: 2 })).toBeVisible();
  await expectClean(diagnostics, 'the added link');

  // A researcher may read every media and gets the reason, not a control.
  const harness = readHarness();
  runCli(
    ensureCli(),
    [
      'user',
      'add',
      '-d',
      harness.dbPath,
      '--email',
      'e2e-researcher@example.com',
      '--name',
      'E2E Researcher',
      '--password',
      'e2e-password',
      '--role',
      'researcher',
    ],
    harness.workDir,
  );
  // Leave the administrator's session; signed out, the item address shows the
  // sign-in panel, and signing in opens the item again.
  await page.context().clearCookies();
  await page.reload();
  const form = page.getByRole('form', { name: 'Sign in' });
  await form.getByLabel('Email').fill('e2e-researcher@example.com');
  await form.getByLabel('Password').fill('e2e-password');
  await form.getByRole('button', { name: 'Sign in' }).click();
  await expect(page.getByText('Signed in as E2E Researcher')).toBeVisible();
  diagnostics.reset();
  await page.goto(itemUrl);
  await expect(page.getByRole('heading', { name: 'Media (3)', level: 2 })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Add media' })).toHaveCount(0);
  await expect(
    page.getByText('You can view these media. Only people who may edit this item can add media.'),
  ).toBeVisible();
  await expectClean(diagnostics, 'a researcher reading the item');
});
