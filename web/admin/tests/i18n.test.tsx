/**
 * i18n tests: the translator, a missing key failing, the locale store
 * persisting, and the shell switch changing what renders.
 *
 * The catalogues are `Catalogue` typed, so a missing key is a `tsc` error and
 * the `npm run typecheck` gate catches it. These tests cover the runtime guard
 * that backs that type, and the behaviour a user sees.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  CATALOGUES,
  DEFAULT_LOCALE,
  LOCALES,
  LOCALE_STORAGE,
  MESSAGE_KEYS,
  MissingMessageError,
  assertComplete,
  createTranslator,
  en,
  missingKeys,
  parseLocale,
} from '../src/i18n';
import { Shell } from '../src/app/shell';
import { jsonResponse } from './stub';

afterEach(() => {
  window.localStorage.clear();
  vi.resetModules();
  window.location.hash = '';
  document.documentElement.lang = 'en';
});

describe('translator', () => {
  it('translator_reads_the_locale_catalogue', () => {
    expect(createTranslator('en').t('page.items')).toBe('Items');
    expect(createTranslator('es').t('page.items')).toBe('Elementos');
    expect(createTranslator('es').locale).toBe('es');
  });

  it('translator_fills_named_placeholders', () => {
    const en = createTranslator('en');
    expect(en.t('list.pager', { page: 1, lastPage: 3, total: 60, resource: 'items' })).toBe(
      'Page 1 of 3 (60 items) ',
    );
    expect(en.t('route.invalidItemId', { param: 'abc' })).toBe('"abc" is not a valid item id.');
    // A placeholder with no value stays as written rather than becoming `undefined`.
    expect(en.t('list.pager', { page: 1 })).toContain('{lastPage}');
  });

  it('translator_formats_numbers_in_the_locale', () => {
    expect(createTranslator('en').number(5989397)).toBe('5,989,397');
    expect(createTranslator('es').number(5989397)).toBe('5.989.397');
  });
});

describe('catalogue completeness', () => {
  it('every_shipped_locale_catalogue_has_every_message', () => {
    for (const locale of LOCALES) {
      expect(missingKeys(CATALOGUES[locale])).toEqual([]);
      expect(() => {
        assertComplete(locale, CATALOGUES[locale]);
      }).not.toThrow();
    }
  });

  it('a_missing_key_fails_with_its_message_id', () => {
    const partial: Record<string, string> = { ...en };
    delete partial['app.title'];
    delete partial['page.items'];

    expect(missingKeys(partial)).toEqual(['app.title', 'page.items']);
    expect(() => {
      assertComplete('en', partial);
    }).toThrow(MissingMessageError);

    try {
      assertComplete('en', partial);
    } catch (error) {
      expect(error instanceof MissingMessageError).toBe(true);
      if (error instanceof MissingMessageError) {
        expect(error.keys).toEqual(['app.title', 'page.items']);
      }
    }
  });
});

describe('locale parsing', () => {
  it('a_bare_tag_and_a_region_tag_both_narrow_to_a_locale', () => {
    expect(parseLocale('en')).toBe('en');
    expect(parseLocale('es')).toBe('es');
    expect(parseLocale('es_419')).toBe('es');
    expect(parseLocale('en_US')).toBe('en');
  });

  it('an_unknown_tag_reads_as_the_default_locale', () => {
    expect(parseLocale('de_DE')).toBe(DEFAULT_LOCALE);
    expect(parseLocale('')).toBe(DEFAULT_LOCALE);
  });
});

describe('locale store', () => {
  it('set_locale_stores_the_choice_and_the_store_reads_it_back', async () => {
    const store = await import('../src/i18n/store');
    store.setLocale('es');

    expect(window.localStorage.getItem(LOCALE_STORAGE)).toBe('es');
    expect(store.currentLocale()).toBe('es');
  });

  it('a_stored_locale_survives_a_reload', async () => {
    window.localStorage.setItem(LOCALE_STORAGE, 'es');
    vi.resetModules();

    const reloaded = await import('../src/i18n/store');

    expect(reloaded.currentLocale()).toBe('es');
  });

  it('set_locale_writes_the_document_language', async () => {
    const store = await import('../src/i18n/store');

    store.setLocale('es');
    expect(document.documentElement.lang).toBe('es');

    store.setLocale('en');
    expect(document.documentElement.lang).toBe('en');
  });

  it('an_unknown_stored_locale_reads_as_the_default', async () => {
    window.localStorage.setItem(LOCALE_STORAGE, 'kl_GL');
    vi.resetModules();

    const reloaded = await import('../src/i18n/store');

    expect(reloaded.currentLocale()).toBe(DEFAULT_LOCALE);
  });
});

describe('shell locale switch', () => {
  it('switching_locale_changes_the_rendered_labels_and_persists', async () => {
    window.location.hash = '';
    vi.stubGlobal(
      'fetch',
      vi.fn<typeof fetch>(() =>
        Promise.resolve(jsonResponse({ errors: { error: 'offline' } }, { status: 503 })),
      ),
    );

    render(<Shell />);
    await screen.findByText('Romeka S admin');

    fireEvent.change(screen.getByLabelText('Language'), { target: { value: 'es' } });

    expect(await screen.findByText('Administración de Romeka S')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Elementos' })).toBeInTheDocument();
    expect(window.localStorage.getItem(LOCALE_STORAGE)).toBe('es');

    fireEvent.change(screen.getByLabelText('Idioma'), { target: { value: 'en' } });

    expect(await screen.findByText('Romeka S admin')).toBeInTheDocument();
    expect(window.localStorage.getItem(LOCALE_STORAGE)).toBe('en');
  });
});

describe('the catalogue surface', () => {
  it('the_key_list_and_the_english_catalogue_agree', () => {
    const keys = new Set(MESSAGE_KEYS);
    expect(keys.size).toBe(MESSAGE_KEYS.length);
    expect(keys.size).toBe(Object.keys(en).length);
  });
});
