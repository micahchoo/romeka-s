/**
 * The namespace mechanism: a component that names a namespace renders English
 * at once, suspends rather than shows English while another locale's table is
 * in flight, keeps its state through a locale switch, and hands a failed load
 * to the error boundary.
 *
 * Each test imports a fresh `src/i18n`, so the namespaces it defines are the
 * only ones the store knows.
 */
import { act, fireEvent, render, screen } from '@testing-library/react';
import { Component, Suspense, useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Locale, Table } from '../src/i18n';

type I18n = typeof import('../src/i18n');

const en = { 'sample.greeting': 'Good morning', 'sample.count': '{count} items' } as const;
type SampleKey = keyof typeof en;
const es: Table<SampleKey> = { 'sample.greeting': 'Buenos días', 'sample.count': '{count} elementos' };

/** Let pending loads and React's retries run. */
const settle = (): Promise<void> =>
  new Promise((resolve) => {
    setTimeout(resolve, 0);
  });

/** A loader that answers only when the test opens it. */
const gated = <T,>(value: T) => {
  let open: () => void = () => undefined;
  let fail: (error: Error) => void = () => undefined;
  const promise = new Promise<T>((resolve, reject) => {
    open = () => {
      resolve(value);
    };
    fail = reject;
  });
  return { load: () => promise, open: () => { open(); }, fail: (error: Error) => { fail(error); } };
};

const loadersWith = (load: () => Promise<Table<SampleKey>>) => {
  const others: Record<Exclude<Locale, 'en'>, () => Promise<Table<SampleKey>>> = {
    es: load,
    hi: load,
    mr: load,
    ml: load,
    ta: load,
    kn: load,
    te: load,
    ne: load,
  };
  return others;
};

class Boundary extends Component<{ readonly children: ReactNode }, { readonly failed: boolean }> {
  override state = { failed: false };
  static getDerivedStateFromError(): { failed: boolean } {
    return { failed: true };
  }
  override render(): ReactNode {
    return this.state.failed ? <p role="alert">failed</p> : this.props.children;
  }
}

let i18n: I18n;

beforeEach(async () => {
  vi.resetModules();
  i18n = await import('../src/i18n');
});

afterEach(async () => {
  await act(() => i18n.setLocale('en'));
  window.localStorage.clear();
});

describe('a namespace', () => {
  it('renders_english_at_once_with_no_fallback', () => {
    const gate = gated(es);
    const sample = i18n.defineNamespace('sample', en, loadersWith(gate.load));
    const Greeting = (): ReactElement => {
      const { t } = i18n.useTranslator(sample);
      return <p>{t('sample.greeting')} {t('app.title', { title: 'Romeka S' })}</p>;
    };

    render(
      <Suspense fallback={<p>waiting</p>}>
        <Greeting />
      </Suspense>,
    );

    expect(screen.getByText('Good morning Romeka S admin')).toBeInTheDocument();
  });

  it('suspends_in_another_locale_until_its_table_arrives_and_never_shows_english', async () => {
    await i18n.setLocale('es');
    const gate = gated(es);
    const sample = i18n.defineNamespace('sample', en, loadersWith(gate.load));
    const Greeting = (): ReactElement => {
      const { t } = i18n.useTranslator(sample);
      return <p>{t('sample.count', { count: 3 })}</p>;
    };

    // An awaited act, so React delivers the retry once the table arrives.
    await act(async () => {
      render(
        <Suspense fallback={<p>waiting</p>}>
          <Greeting />
        </Suspense>,
      );
      await settle();
    });

    expect(screen.getByText('waiting')).toBeInTheDocument();
    expect(screen.queryByText('3 items')).toBeNull();
    await act(async () => {
      gate.open();
      await settle();
    });
    expect(screen.getByText('3 elementos')).toBeInTheDocument();
  });

  it('a_locale_switch_waits_for_every_defined_table_and_keeps_a_draft', async () => {
    const gate = gated(es);
    const sample = i18n.defineNamespace('sample', en, loadersWith(gate.load));
    const Draft = (): ReactElement => {
      const { t } = i18n.useTranslator(sample);
      const [text, setText] = useState('');
      return (
        <label>
          {t('sample.greeting')}
          <input value={text} onChange={(event) => { setText(event.target.value); }} />
        </label>
      );
    };
    render(
      <Suspense fallback={<p>waiting</p>}>
        <Draft />
      </Suspense>,
    );
    fireEvent.change(screen.getByLabelText('Good morning'), { target: { value: 'my draft' } });

    let switched = false;
    const switching = i18n.setLocale('es').then(() => {
      switched = true;
    });
    // The catalogue has arrived, the table has not: the page stays English, mounted.
    await act(async () => {
      await Promise.resolve();
    });
    expect(switched).toBe(false);
    expect(i18n.currentLocale()).toBe('en');
    expect(screen.getByLabelText('Good morning')).toHaveValue('my draft');

    await act(async () => {
      gate.open();
      await switching;
    });
    expect(screen.queryByText('waiting')).toBeNull();
    expect(screen.getByLabelText('Buenos días')).toHaveValue('my draft');
  });

  it('a_switch_whose_table_fails_rejects_and_leaves_the_locale', async () => {
    const gate = gated(es);
    i18n.defineNamespace('sample', en, loadersWith(gate.load));
    gate.fail(new Error('chunk offline'));

    await expect(i18n.setLocale('es')).rejects.toThrow('chunk offline');
    expect(i18n.currentLocale()).toBe('en');
    expect(window.localStorage.getItem(i18n.LOCALE_STORAGE)).toBeNull();
  });

  it('a_route_whose_table_fails_reaches_the_error_boundary', async () => {
    await i18n.setLocale('es');
    const offline = (): Promise<Table<SampleKey>> => Promise.reject(new Error('chunk offline'));
    const sample = i18n.defineNamespace('sample', en, loadersWith(offline));
    const Greeting = (): ReactElement => {
      const { t } = i18n.useTranslator(sample);
      return <p>{t('sample.greeting')}</p>;
    };
    const quiet = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    try {
      await act(async () => {
        render(
          <Boundary>
            <Suspense fallback={<p>waiting</p>}>
              <Greeting />
            </Suspense>
          </Boundary>,
        );
        await settle();
      });
      expect(screen.getByRole('alert')).toHaveTextContent('failed');
      expect(screen.queryByText('Good morning')).toBeNull();
    } finally {
      quiet.mockRestore();
    }
  });

  it('an_incomplete_table_is_refused_with_its_missing_ids', async () => {
    const short: Readonly<Record<string, string>> = { 'sample.greeting': 'Buenos días' };
    const load = (): Promise<Readonly<Record<string, string>>> => Promise.resolve(short);
    i18n.defineNamespace<string>('short', en, {
      es: load, hi: load, mr: load, ml: load, ta: load, kn: load, te: load, ne: load,
    });

    await expect(i18n.setLocale('es')).rejects.toThrow('short:sample.count');
    expect(i18n.currentLocale()).toBe('en');
  });

  it('a_first_read_namespace_is_fetched_only_once_a_component_reads_it', async () => {
    const load = vi.fn(() => Promise.resolve(es));
    const sample = i18n.defineNamespace('sample', en, loadersWith(load), { onFirstRead: true });

    await i18n.setLocale('es');
    expect(load).not.toHaveBeenCalled();

    const Greeting = (): ReactElement => {
      const { t } = i18n.useTranslator(sample);
      return <p>{t('sample.greeting')}</p>;
    };
    await act(async () => {
      render(
        <Suspense fallback={<p>waiting</p>}>
          <Greeting />
        </Suspense>,
      );
      await settle();
    });
    expect(screen.getByText('Buenos días')).toBeInTheDocument();
    expect(load).toHaveBeenCalledTimes(1);

    // Once read, a switch fetches it before switching, as every other namespace.
    await act(() => i18n.setLocale('hi'));
    expect(load).toHaveBeenCalledTimes(2);
  });

  it('types_a_key_from_an_undeclared_namespace_as_an_error', () => {
    const sample = i18n.defineNamespace('sample', en, loadersWith(() => Promise.resolve(es)));
    const Keys = (): ReactElement => {
      const shell = i18n.useTranslator();
      const both = i18n.useTranslator(sample);
      // @ts-expect-error `sample.greeting` is not a shell key.
      const wrong = shell.t('sample.greeting');
      return <p>{both.t('sample.greeting')}{wrong}</p>;
    };
    render(<Keys />);
    expect(screen.getByText(/Good morning/)).toBeInTheDocument();
  });
});
