/**
 * Server error text in the admin's locale. The API answers in upstream's
 * English, which is the msgid, and the client translates it at the fetch
 * boundary by matching it against `src/generated/serverMessages.json`
 * (`crates/omeka-api/src/messages.rs`).
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import generated from '../src/generated/serverMessages.json';
import { setLocale, serverTranslator } from '../src/i18n';
import { es } from '../src/i18n/locales/es';
import { esServer } from '../src/i18n/locales/es.server';
import { createServerTranslator } from '../src/i18n/serverMessages';
import type { ServerCatalogue } from '../src/i18n/serverMessages';
import { VocabulariesPage } from '../src/pages/vocabularies/page';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse } from './stub';

afterEach(async () => {
  await setLocale('en');
  window.localStorage.clear();
  window.location.hash = '';
});

const referenceRoutes: Readonly<Record<string, () => Response>> = {
  '/api/users': () => jsonResponse(loadFixture('users-page.json')),
  '/api/vocabularies': () => jsonResponse(loadFixture('vocabularies-page.json')),
};

describe('server messages in the active locale', () => {
  it('a_validation_error_from_the_server_renders_in_spanish_with_its_value', async () => {
    await setLocale('es');
    installFetch((url, init) => {
      if (url.pathname === '/api/vocabularies' && init?.method === 'POST') {
        return jsonResponse(
          { errors: { 'o:prefix': ['The prefix "bibo" is already taken.'] } },
          { status: 422 },
        );
      }
      return (referenceRoutes[url.pathname] ?? (() => jsonResponse({}, { status: 404 })))();
    });
    render(<VocabulariesPage param="new" />);

    fireEvent.change(await screen.findByLabelText(es['detail.namespaceUri']), {
      target: { value: 'http://purl.org/ontology/bibo/' },
    });
    fireEvent.change(screen.getByLabelText(es['column.prefix']), { target: { value: 'bibo' } });
    fireEvent.change(screen.getByLabelText(es['field.label']), {
      target: { value: 'Bibliographic' },
    });
    fireEvent.click(screen.getByRole('button', { name: es['vocabularies.add'] }));

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'El prefijo "bibo" ya está en uso. (HTTP 422)',
    );
  });

  it('a_substituted_value_survives_the_translation', async () => {
    const translate = await serverTranslator('es');
    expect(translate('The slug "my-site" is already taken.')).toBe(
      'La ruta "my-site" ya está en uso.',
    );
    expect(translate('Omeka\\Entity\\Item entity with criteria {"id":"9"} not found')).toBe(
      'No se ha encontrado la entidad Omeka\\Entity\\Item con el criterio {"id":"9"}',
    );
    expect(translate('Resource not found with id $1.')).toBe(
      'El recurso con identidad $1 no se ha encontrado.',
    );
  });

  it('unknown_text_passes_through_unchanged', async () => {
    const translate = await serverTranslator('es');
    expect(translate('EOF while parsing a value at line 1 column 0')).toBe(
      'EOF while parsing a value at line 1 column 0',
    );
    expect(translate('The prefix "bibo" is already taken. Extra')).toBe(
      'The prefix "bibo" is already taken. Extra',
    );
  });

  it('english_shows_the_server_text_as_it_arrives', async () => {
    const translate = await serverTranslator('en');
    expect(translate('The slug "my-site" is already taken.')).toBe(
      'The slug "my-site" is already taken.',
    );
  });

  it('every_pattern_matches_back_to_its_own_key', () => {
    // A catalogue whose every message is its own key shows which pattern won.
    const keys = Object.fromEntries(Object.keys(esServer).map((key) => [key, key]));
    const ownKey: ServerCatalogue = { ...esServer, ...keys };
    const translate = createServerTranslator(ownKey);
    for (const [key, pattern] of Object.entries(generated.messages)) {
      expect(translate(pattern.replace(/\{[a-z]+\}/g, 'x'))).toBe(key);
    }
  });

  it('every_spanish_message_keeps_the_holes_of_its_pattern', () => {
    const holes = (text: string): string[] => [...text.matchAll(/\{[a-z]+\}/g)].map(String).sort();
    for (const [key, pattern] of Object.entries(generated.messages)) {
      const translated: unknown = Object.entries(esServer).find(([k]) => k === key)?.[1];
      expect(typeof translated, key).toBe('string');
      expect(holes(String(translated)), key).toEqual(holes(pattern));
    }
  });
});
