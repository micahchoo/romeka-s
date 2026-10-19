/**
 * No English flash: a page opened in Spanish while its namespace's Spanish
 * table is still in flight shows its Suspense fallback and never one English
 * string, then renders Spanish once the table arrives.
 *
 * The Columns page reads the `admin` and `common` namespaces. The `admin`
 * Spanish table is held back until the test opens the gate, the way
 * `tests/theme.test.tsx` holds back its Hindi table. The page module is
 * imported only after the switch to Spanish, as a route chunk is, so the switch
 * does not wait for the table and the page itself must.
 */
import { act, render, screen } from '@testing-library/react';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse } from './stub';
import { Suspense } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { setLocale } from '../src/i18n';

const spanishAdmin = vi.hoisted(() => {
  let open: () => void = () => undefined;
  const gate = new Promise<void>((resolve) => {
    open = resolve;
  });
  return {
    gate,
    open: () => {
      open();
    },
  };
});
vi.mock('../src/i18n/namespaces/admin/locales/es', async (importOriginal) => {
  await spanishAdmin.gate;
  return importOriginal();
});

// The Spanish vocabulary labels, held back the same way.
const spanishLabels = vi.hoisted(() => {
  let open: () => void = () => undefined;
  const gate = new Promise<void>((resolve) => {
    open = resolve;
  });
  return {
    gate,
    open: () => {
      open();
    },
  };
});
vi.mock('../src/i18n/locales/vocabulary/labels/es.json', async (importOriginal) => {
  await spanishLabels.gate;
  return importOriginal();
});

/** Let pending loads and React's retries run. */
const settle = (): Promise<void> =>
  new Promise((resolve) => {
    setTimeout(resolve, 0);
  });

afterEach(async () => {
  await act(() => setLocale('en'));
});

/** Every English string of the tables that differs from its Spanish, worth searching for. */
const englishOnly = (
  english: Readonly<Record<string, string>>,
  spanish: Readonly<Record<string, string>>,
): readonly string[] =>
  Object.entries(english)
    .filter(([key, text]) => text !== spanish[key] && /[A-Za-z]{3}/.test(text))
    .map(([, text]) => text.replace(/\{[A-Za-z]+\}/g, '').trim())
    .filter((text) => text.length > 3);

describe('a page in Spanish', () => {
  it('shows no English while its table is in flight, then Spanish', async () => {
    await setLocale('es');
    // Every text the document ever holds, so a flash between two checks still counts.
    const seen: string[] = [];
    const observer = new MutationObserver(() => {
      seen.push(document.body.textContent);
    });
    observer.observe(document.body, { childList: true, subtree: true, characterData: true });
    try {
      // The route chunk arrives after the switch, so its namespaces are defined in Spanish.
      const [{ ColumnsPage }, admin, common, commonSpanish] = await Promise.all([
        import('../src/pages/columns/page'),
        import('../src/i18n/namespaces/admin'),
        import('../src/i18n/namespaces/common'),
        import('../src/i18n/namespaces/common/locales/es'),
      ]);
      await act(async () => {
        render(
          <Suspense fallback={<p role="status">Cargando…</p>}>
            <ColumnsPage />
          </Suspense>,
        );
        await settle();
      });
      expect(screen.getByRole('status')).toHaveTextContent('Cargando…');

      spanishAdmin.open();
      const { es: adminSpanish } = await import('../src/i18n/namespaces/admin/locales/es');
      expect(await screen.findByText(adminSpanish['columns.legend'])).toBeInTheDocument();
      expect(screen.getByLabelText(adminSpanish['columns.resourceType'])).toBeInTheDocument();

      const english = [
        ...englishOnly(admin.en, adminSpanish),
        ...englishOnly(common.en, commonSpanish.es),
      ];
      seen.push(document.body.textContent);
      const flashed = english.filter((text) => seen.some((snapshot) => snapshot.includes(text)));
      expect(flashed).toEqual([]);
    } finally {
      observer.disconnect();
    }
  });

  it('shows no English vocabulary label while the label table is in flight', async () => {
    await setLocale('es');
    const routes: Readonly<Record<string, string>> = {
      '/api/properties': 'properties-page.json',
      '/api/vocabularies': 'vocabularies-page.json',
      '/api/resource_classes': 'resource-classes-page.json',
      '/api/resource_templates': 'resource-templates-page.json',
      '/api/users': 'users-page.json',
    };
    installFetch((url) => {
      const fixture = routes[url.pathname];
      return fixture === undefined
        ? jsonResponse({ errors: { error: 'unexpected' } }, { status: 404 })
        : jsonResponse(loadFixture(fixture));
    });
    const seen: string[] = [];
    const observer = new MutationObserver(() => {
      seen.push(document.body.textContent);
    });
    observer.observe(document.body, { childList: true, subtree: true, characterData: true });
    try {
      const { PropertiesPage } = await import('../src/pages/properties/page');
      // The list renders once its fetch answers, inside the act; the table
      // arrives a little later, so the list must wait for it there.
      setTimeout(spanishLabels.open, 200);
      await act(async () => {
        render(
          <Suspense fallback={<p role="status">Cargando…</p>}>
            <PropertiesPage param={null} />
          </Suspense>,
        );
        await settle();
      });
      expect(await screen.findByRole('cell', { name: 'Título' })).toBeInTheDocument();
      // dcterms:title's built-in English never painted in its place.
      expect(seen.filter((snapshot) => /\bTitle\b/.test(snapshot))).toEqual([]);
    } finally {
      observer.disconnect();
    }
  });
});
