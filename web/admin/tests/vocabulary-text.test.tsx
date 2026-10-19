/**
 * The built-in vocabularies' labels and comments in the admin's locale, after
 * upstream's `$translate($property->label())` and
 * `alternate_label ?: translated label`.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { loadTermText, setLocale, termText } from '../src/i18n';
import { es } from '../src/i18n/locales/es';
import { ItemsPage } from '../src/pages/items/page';
import { PropertiesPage } from '../src/pages/properties/page';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse } from './stub';

afterEach(async () => {
  await setLocale('en');
  window.localStorage.clear();
});

const routes: Readonly<Record<string, () => Response>> = {
  '/api/items/1': () => jsonResponse(loadFixture('item-1.json')),
  '/api/media': () => jsonResponse(loadFixture('media-page-live.json')),
  '/api/users': () => jsonResponse(loadFixture('users-page.json')),
  '/api/resource_classes': () => jsonResponse(loadFixture('resource-classes-page.json')),
  '/api/resource_templates': () => jsonResponse(loadFixture('resource-templates-page.json')),
  '/api/properties': () => jsonResponse(loadFixture('properties-page.json')),
  '/api/vocabularies': () => jsonResponse(loadFixture('vocabularies-page.json')),
  '/api/item_sets': () => jsonResponse(loadFixture('item-sets-page.json')),
  '/api/data_types': () => jsonResponse(loadFixture('data-types-page.json')),
  // The item form offers the sites an item can join (pages/items/sites.tsx).
  '/api/sites': () => jsonResponse(loadFixture('sites-page.json')),
};

const serve = (): void => {
  installFetch((url) =>
    (routes[url.pathname] ?? (() => jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 })))(),
  );
};

describe('built-in vocabulary text in the active locale', () => {
  it('the_item_editor_shows_titulo_in_spanish_and_an_alternate_label_as_stored', async () => {
    await setLocale('es');
    serve();
    render(<ItemsPage param="1" />);
    fireEvent.click(await screen.findByText(es['items.editLabel']));

    // dcterms:title has no alternate label on the "Photograph" template.
    expect(await screen.findByRole('textbox', { name: 'Título' })).toHaveValue('Fixture Item One');
    // dcterms:description is called "Caption" there, which stays as written.
    expect(screen.getByText('Caption', { selector: '.property-field-label' })).toBeInTheDocument();
    expect(screen.queryByText('Descripción', { selector: '.property-field-label' })).toBeNull();
  });

  it('the_property_browse_lists_built_in_labels_in_spanish', async () => {
    await setLocale('es');
    serve();
    render(<PropertiesPage param={null} />);
    expect(await screen.findByRole('cell', { name: 'Título' })).toBeInTheDocument();
  });

  it('a_label_that_is_not_the_built_in_english_shows_as_stored', async () => {
    await loadTermText('es');
    const text = termText('es');
    expect(text.label('dcterms:title', 'Title')).toBe('Título');
    expect(text.label('dcterms:title', 'Headline')).toBe('Headline');
    expect(text.label('shelf:title', 'Title')).toBe('Title');
    expect(text.comment('dcterms:title', null)).toBeNull();
  });
});
