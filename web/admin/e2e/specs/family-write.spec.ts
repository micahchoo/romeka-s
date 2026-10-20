/**
 * The write surface of the vocabulary family, the assets and the jobs, driven
 * as browser journeys against the running port.
 *
 * A vocabulary, a resource class, a property and a resource template each get a
 * create, an edit and a delete. An asset and a job have no create through the
 * API (`POST /api/assets` needs a multipart file and `POST /api/jobs` needs a
 * `class` the dispatcher sets), so the harness seeds one of each and the spec
 * edits and deletes it.
 */
import { readHarness } from '../harness/state';
import { ASSET_NAME, JOB_CLASS } from '../harness/seed';
import { expect, expectClean, test } from '../fixtures';

/** The vocabulary a new property or class joins. */
const VOCABULARY_LABEL = 'Dublin Core';

test('a vocabulary is created, renamed and deleted', async ({ page, diagnostics }) => {
  const name = 'E2E vocabulary';
  const renamed = `${name} renamed`;

  await page.goto('/#/vocabularies');
  await page.getByRole('link', { name: 'Add vocabulary' }).click();
  await expect(page).toHaveURL(/#\/standards\/vocabularies\/new$/);

  await page.locator('#vocabulary-namespace-uri').fill('http://example.org/e2e/');
  await page.locator('#vocabulary-prefix').fill('e2evocab');
  await page.locator('#vocabulary-label').fill(name);
  await page.getByRole('button', { name: 'Add vocabulary' }).click();

  await expect(page).toHaveURL(/#\/standards\/vocabularies\/\d+$/);
  await expect(page.getByRole('heading', { name })).toBeVisible();
  await expectClean(diagnostics, 'vocabulary create');

  await page.getByRole('button', { name: 'Edit this vocabulary' }).click();
  await page.locator('#vocabulary-label').fill(renamed);
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('[role=status]')).toHaveText('Saved.');
  await expect(page.getByRole('heading', { name: renamed })).toBeVisible();
  await expectClean(diagnostics, 'vocabulary rename');

  await page.getByRole('link', { name: 'Vocabularies' }).click();
  await expect(page.getByRole('link', { name: renamed })).toBeVisible();

  await page.getByRole('link', { name: renamed }).click();
  await page.getByRole('button', { name: 'Delete this vocabulary' }).click();
  await page.getByRole('button', { name: `Delete ${renamed}` }).click();

  await expect(page).toHaveURL(/#\/standards\/vocabularies$/);
  await expect(page.getByRole('link', { name: renamed })).toHaveCount(0);
  await expectClean(diagnostics, 'vocabulary delete');
});

test('a resource class is created, renamed and deleted', async ({ page, diagnostics }) => {
  const localName = 'E2eClass';
  const renamed = 'E2eClass revised';
  const term = `dcterms:${localName}`;

  await page.goto('/#/resource-classes');
  await page.getByRole('link', { name: 'Add resource class' }).click();
  await expect(page).toHaveURL(/#\/standards\/classes\/new$/);

  await page.locator('#member-resource_classes-vocabulary').selectOption({
    label: VOCABULARY_LABEL,
  });
  await page.locator('#member-resource_classes-local-name').fill(localName);
  await page.locator('#member-resource_classes-label').fill(localName);
  await page.getByRole('button', { name: 'Add resource class' }).click();

  await expect(page).toHaveURL(/#\/standards\/classes\/\d+$/);
  await expect(page.getByRole('heading', { name: localName })).toBeVisible();
  await expectClean(diagnostics, 'resource class create');

  await page.getByRole('button', { name: 'Edit this resource class' }).click();
  await page.locator('#member-resource_classes-label').fill(renamed);
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('[role=status]')).toHaveText('Saved.');
  await expect(page.getByRole('heading', { name: renamed })).toBeVisible();
  await expectClean(diagnostics, 'resource class rename');

  await page.getByRole('button', { name: 'Delete this resource class' }).click();
  await page.getByRole('button', { name: `Delete ${renamed}` }).click();

  await expect(page).toHaveURL(/#\/standards\/classes$/);
  await expect(page.getByRole('link', { name: term })).toHaveCount(0);
  await expectClean(diagnostics, 'resource class delete');
});

test('a property is created, renamed and deleted', async ({ page, diagnostics }) => {
  const localName = 'e2eProperty';
  const renamed = 'e2eProperty revised';
  const term = `dcterms:${localName}`;

  await page.goto('/#/properties');
  await page.getByRole('link', { name: 'Add property' }).click();
  await expect(page).toHaveURL(/#\/standards\/properties\/new$/);

  await page.locator('#member-properties-vocabulary').selectOption({ label: VOCABULARY_LABEL });
  await page.locator('#member-properties-local-name').fill(localName);
  await page.locator('#member-properties-label').fill(localName);
  await page.getByRole('button', { name: 'Add property' }).click();

  await expect(page).toHaveURL(/#\/standards\/properties\/\d+$/);
  await expect(page.getByRole('heading', { name: localName })).toBeVisible();
  await expectClean(diagnostics, 'property create');

  await page.getByRole('button', { name: 'Edit this property' }).click();
  await page.locator('#member-properties-label').fill(renamed);
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('[role=status]')).toHaveText('Saved.');
  await expect(page.getByRole('heading', { name: renamed })).toBeVisible();
  await expectClean(diagnostics, 'property rename');

  await page.getByRole('button', { name: 'Delete this property' }).click();
  await page.getByRole('button', { name: `Delete ${renamed}` }).click();

  await expect(page).toHaveURL(/#\/standards\/properties$/);
  await expect(page.getByRole('link', { name: term })).toHaveCount(0);
  await expectClean(diagnostics, 'property delete');
});

test('an item type is created from a kind, renamed and deleted', async ({ page, diagnostics }) => {
  const label = 'E2E letter type';
  const renamed = `${label} renamed`;

  await page.goto('/#/resource-templates');
  await page.getByRole('link', { name: 'New item type' }).click();
  await expect(page).toHaveURL(/#\/resource-templates\/new$/);

  await page.getByLabel('What kind of thing is this?').fill('Letter');
  await page.getByRole('button', { name: 'Use these fields' }).click();
  await page.getByRole('textbox', { name: 'Item type name' }).fill(label);
  await page.getByRole('button', { name: 'Create item type' }).click();

  await expect(page).toHaveURL(/#\/resource-templates\/\d+$/);
  await expect(page.getByRole('textbox', { name: 'Item type name' })).toHaveValue(label);
  await expect(page.getByText('Standard class: Letter')).toBeVisible();
  await expectClean(diagnostics, 'item type create');

  await page.getByRole('textbox', { name: 'Item type name' }).fill(renamed);
  await page.getByRole('button', { name: 'Save item type' }).click();
  await expect(page.locator('[role=status]').filter({ hasText: 'Saved.' })).toBeVisible();
  await expectClean(diagnostics, 'item type rename');

  await page.getByRole('button', { name: 'Delete this item type' }).click();
  await page.getByRole('button', { name: `Delete ${renamed}` }).click();

  await expect(page).toHaveURL(/#\/resource-templates$/);
  await expect(page.getByRole('link', { name: renamed })).toHaveCount(0);
  await expectClean(diagnostics, 'item type delete');
});

test('a vocabulary edit keeps the properties it already has', async ({ page, diagnostics }) => {
  const vocabulary = 'E2E keep vocabulary';
  const propertyLabel = 'Kept property';
  const localName = 'e2eKeptProperty';

  await page.goto('/#/vocabularies/new');
  await page.locator('#vocabulary-namespace-uri').fill('http://example.org/keep/');
  await page.locator('#vocabulary-prefix').fill('e2ekeep');
  await page.locator('#vocabulary-label').fill(vocabulary);
  await page.getByRole('button', { name: 'Add vocabulary' }).click();
  await expect(page).toHaveURL(/#\/standards\/vocabularies\/\d+$/);
  await diagnostics.idle();
  const vocabularyUrl = page.url();

  await page.getByRole('link', { name: 'Properties' }).click();
  await page.getByRole('link', { name: 'Add property' }).click();
  await page.locator('#member-properties-vocabulary').selectOption({ label: vocabulary });
  await page.locator('#member-properties-local-name').fill(localName);
  await page.locator('#member-properties-label').fill(propertyLabel);
  await page.getByRole('button', { name: 'Add property' }).click();
  await expect(page).toHaveURL(/#\/standards\/properties\/\d+$/);
  await diagnostics.idle();

  // The property is one of the vocabulary's members.
  await page.goto(vocabularyUrl);
  await expect(page.getByRole('link', { name: propertyLabel })).toBeVisible();

  // The edit is a PATCH, so it must not carry the member lists a full PUT
  // would have to repeat. A full write that omits them removes every member.
  await page.getByRole('button', { name: 'Edit this vocabulary' }).click();
  await page.locator('#vocabulary-label').fill('E2E keep vocabulary renamed');
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('[role=status]')).toHaveText('Saved.');
  await diagnostics.idle();

  await page.goto(vocabularyUrl);
  await expect(page.getByRole('link', { name: propertyLabel })).toBeVisible();
  await expectClean(diagnostics, 'vocabulary edit keeps its members');

  await page.getByRole('button', { name: 'Delete this vocabulary' }).click();
  await page.getByRole('button', { name: 'Delete E2E keep vocabulary renamed' }).click();
  await expect(page).toHaveURL(/#\/standards\/vocabularies$/);
});

test('the seeded asset is renamed and deleted', async ({ page, diagnostics }) => {
  const renamed = `${ASSET_NAME} revised`;

  await page.goto('/#/assets');
  await page.getByRole('link', { name: ASSET_NAME }).click();
  await expect(page).toHaveURL(/#\/assets\/\d+$/);
  await expect(page.getByRole('heading', { name: ASSET_NAME })).toBeVisible();

  await page.getByRole('button', { name: 'Edit this asset' }).click();
  await page.locator('#asset-name').fill(renamed);
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('[role=status]')).toHaveText('Saved.');
  await expect(page.getByRole('heading', { name: renamed })).toBeVisible();
  await expectClean(diagnostics, 'asset rename');

  await page.getByRole('button', { name: 'Delete this asset' }).click();
  await page.getByRole('button', { name: `Delete ${renamed}` }).click();

  await expect(page).toHaveURL(/#\/assets$/);
  await expect(page.getByRole('link', { name: renamed })).toHaveCount(0);
  await expectClean(diagnostics, 'asset delete');
});

test('the seeded job is updated and deleted', async ({ page, diagnostics }) => {
  const { jobId } = readHarness();

  await page.goto('/#/jobs');
  await page.getByRole('link', { name: String(jobId), exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`#/jobs/${String(jobId)}$`));
  await expect(page.getByText(JOB_CLASS)).toBeVisible();
  // The seed writes no status, so this proves the null `o:status` the wire sends
  // reaches the view instead of failing the parse. The list behind this route
  // renders the same class string, so the assertion names the detail's value.
  await expect(page.getByRole('definition').filter({ hasText: '[Unknown]' })).toHaveText(
    '[Unknown]',
  );

  await page.getByRole('button', { name: 'Update this job' }).click();
  await page.getByRole('button', { name: 'Update', exact: true }).click();
  await expect(page.locator('[role=status]')).toHaveText('Saved.');
  // The adapter hydrates no member, so the update leaves the row alone.
  await expect(page.getByText(JOB_CLASS)).toBeVisible();
  await expect(page.getByRole('definition').filter({ hasText: '[Unknown]' })).toHaveText(
    '[Unknown]',
  );
  await expectClean(diagnostics, 'job update');

  await page.getByRole('button', { name: 'Delete this job' }).click();
  await page.getByRole('button', { name: `Delete Job #${String(jobId)}` }).click();

  await expect(page).toHaveURL(/#\/jobs$/);
  await expect(page.getByRole('link', { name: String(jobId), exact: true })).toHaveCount(0);
  await expectClean(diagnostics, 'job delete');
});
