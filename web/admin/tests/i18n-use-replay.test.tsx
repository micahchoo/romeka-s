/**
 * A component that reads one ready namespace, then suspends on a second one,
 * then calls a hook it has never called before. React replays a component that
 * suspended on a promise which settles before it yields, and it switches to
 * its mount dispatcher only inside `use()`. So a replay that skips `use()`
 * because the table has meanwhile arrived renders the new hook as an update:
 * "Update hook called on initial render" (React error 467). React recovers by
 * rendering the whole root again, and reports error 520. That was the live
 * items list in Hindi: `useTranslator` ready, `useTermLabels` in flight, then
 * its `useMemo`.
 */
import { act } from '@testing-library/react';
import { startTransition, Suspense, useMemo } from 'react';
import { createRoot } from 'react-dom/client';
import type { ReactElement } from 'react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import type { Locale, Table } from '../src/i18n';

type I18n = typeof import('../src/i18n');

const readyEn = { 'ready.word': 'Ready' } as const;
const lateEn = { 'late.word': 'Late' } as const;

const everyLocale = <K extends string>(table: Table<K>): Record<Exclude<Locale, 'en'>, () => Promise<Table<K>>> => {
  const load = (): Promise<Table<K>> => Promise.resolve(table);
  return { es: load, hi: load, mr: load, ml: load, ta: load, kn: load, te: load, ne: load };
};

let i18n: I18n;

beforeEach(async () => {
  vi.resetModules();
  i18n = await import('../src/i18n');
});

afterEach(async () => {
  await act(() => i18n.setLocale('en'));
  window.localStorage.clear();
});

it('a_replay_after_a_table_arrives_mounts_the_hooks_that_follow_it', async () => {
  const ready = i18n.defineNamespace('ready', readyEn, everyLocale({ 'ready.word': 'Listo' }));
  // Fetched on first read, so the locale switch leaves it in flight for the render.
  const late = i18n.defineNamespace('late', lateEn, everyLocale({ 'late.word': 'Tarde' }), {
    onFirstRead: true,
  });
  await act(() => i18n.setLocale('es'));

  const Page = (): ReactElement => {
    const { t } = i18n.useTranslator(ready);
    const { t: tl } = i18n.useTranslator(late);
    const both = useMemo(() => `${t('ready.word')} ${tl('late.word')}`, [t, tl]);
    return <p>{both}</p>;
  };

  // A concurrent render, outside act: only the concurrent work loop replays a
  // component whose promise settled while it waited, as a lazy route's retry does.
  const actEnvironment: unknown = Reflect.get(globalThis, 'IS_REACT_ACT_ENVIRONMENT');
  Reflect.set(globalThis, 'IS_REACT_ACT_ENVIRONMENT', false);
  const container = document.createElement('div');
  document.body.append(container);
  const recovered: unknown[] = [];
  const root = createRoot(container, {
    onRecoverableError: (error: unknown) => {
      recovered.push(error);
    },
  });
  try {
    startTransition(() => {
      root.render(
        <Suspense fallback={<p>waiting</p>}>
          <Page />
        </Suspense>,
      );
    });
    await vi.waitFor(() => {
      expect(container.textContent).toBe('Listo Tarde');
    });
  } finally {
    root.unmount();
    container.remove();
    Reflect.set(globalThis, 'IS_REACT_ACT_ENVIRONMENT', actEnvironment);
  }

  expect(recovered).toEqual([]);
});
