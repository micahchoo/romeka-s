/**
 * The frame of one import (`#/import/<id>/<mode>/<screen>`): the mode switch
 * lands on the screen that shows the same thing and keeps every answer, the
 * steps mark where the person is, the status line counts what the saved
 * answers make, and a save someone else overtook shows a plain reload prompt.
 *
 * The screen bodies are stand-ins that read and edit the plan through
 * `useDraft()`, the seam every real body uses.
 */
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { asRecord } from '../src/api/parse';
import { ImportPage } from '../src/pages/import/page';
import { SAVE_DELAY_MS } from '../src/pages/import/plan-store';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, requestBody, requestsFor, route } from './stub';
import type { Handler } from './stub';

const CaseFold = vi.hoisted(() => ({ label: 'Names differ only in capitals' }));

vi.mock('../src/pages/import/simple/bodies', async () => {
  const { useDraft } = await import('../src/pages/import/draft-context');
  const { seriesOf, withSeries } = await import('../src/pages/import/series');
  const Found = () => {
    const draft = useDraft();
    return (
      <label>
        <input
          type="checkbox"
          checked={seriesOf(draft.plan, 'all').grouping.caseFold}
          disabled={!draft.editable}
          onChange={(event) => {
            const on = event.target.checked;
            draft.change((plan) => withSeries(plan, 'all', (series) => ({ ...series, grouping: { ...series.grouping, caseFold: on } })));
          }}
        />
        {CaseFold.label}
      </label>
    );
  };
  return { SIMPLE_BODIES: { found: Found } };
});

vi.mock('../src/pages/import/advanced/bodies', async () => {
  const { useDraft } = await import('../src/pages/import/draft-context');
  const { seriesOf } = await import('../src/pages/import/series');
  const Structure = () => <p>caseFold: {String(seriesOf(useDraft().plan, 'all').grouping.caseFold)}</p>;
  return { ADVANCED_BODIES: { structure: Structure } };
});

const fixture = (name: string): Record<string, unknown> => asRecord(loadFixture(`import/${name}`), []);
const PATH = '/api/romeka/imports/1';

const preview: Handler = () =>
  jsonResponse({
    ...fixture('preview-check.json'),
    flags: fixture('preview-flags.json')['flags'],
    identical: fixture('preview-identical.json')['identical'],
    entries: fixture('preview-entries.json')['entries'],
    unpaired: fixture('preview-unpaired.json')['unpaired'],
  });

const savedAs = (init: RequestInit | undefined, revision: number): Response => {
  const sent = typeof init?.body === 'string' ? asRecord(JSON.parse(init.body), []) : {};
  return jsonResponse({ ...fixture('import-draft.json'), plan: sent['plan'], revision });
};

const draftRoutes = (onPut: (init: RequestInit | undefined) => Response): Handler =>
  route({
    [PATH]: (_url, init) => (init?.method === 'PUT' ? onPut(init) : jsonResponse(fixture('import-draft.json'))),
    [`${PATH}/preview`]: preview,
  });

const at = (rest: readonly string[]) => <ImportPage param="1" rest={['1', ...rest]} />;

afterEach(() => {
  vi.useRealTimers();
  window.location.hash = '';
});

describe('the frame', () => {
  it('marks the step, names the screen and counts what the saved answers make', async () => {
    installFetch(draftRoutes(() => jsonResponse({}, { status: 500 })));
    render(at(['simple', 'example']));
    expect(await screen.findByRole('heading', { name: 'Show us one example' })).toBeInTheDocument();
    const steps = screen.getByRole('list', { name: 'Steps' });
    expect(within(steps).getAllByRole('link').map((link) => link.textContent)).toEqual(['Structure', 'Check', 'Add']);
    // The example is a sub-screen of "What we found", which is one of Structure.
    expect(within(steps).getByRole('link', { name: 'Structure' })).toHaveAttribute('aria-current', 'step');
    expect(screen.getByRole('link', { name: 'Back' })).toHaveAttribute('href', '#/import/1/simple/found');
    expect(screen.queryByRole('link', { name: 'Next' })).not.toBeInTheDocument();
    expect(await screen.findByText('10 items so far.')).toBeInTheDocument();
    const line = screen.getByText('10 items so far.').closest('p');
    // Open questions: date-form, name-encoding, cells-count, no-file; identical files, files no item takes, items with no title.
    expect(line).toHaveTextContent('10 items so far. 1 left out. 7 questions open. Nothing is in Omeka until you add it.');
    expect(within(line ?? document.body).getByRole('link')).toHaveAttribute('href', '#/import/1/simple/check');
  });

  it('opens a draft with files on Structure, and a started import on Add', async () => {
    const fetchMock = installFetch(
      route({
        [PATH]: () => jsonResponse(fixture('import-draft.json')),
        '/api/romeka/imports/6': () => jsonResponse(fixture('import-done.json')),
        [`${PATH}/preview`]: preview,
        '/api/romeka/imports/6/preview': preview,
      }),
    );
    const { unmount } = render(<ImportPage param="1" rest={['1']} />);
    expect(await screen.findByRole('heading', { name: 'How your archive becomes items and sets' })).toBeInTheDocument();
    unmount();
    render(<ImportPage param="6" rest={['6']} />);
    expect(await screen.findByRole('heading', { name: 'Add them' })).toBeInTheDocument();
    expect(screen.getByText(/This import has started, so its answers cannot change/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Make a copy' })).toBeInTheDocument();
    await act(async () => {});
    expect(requestsFor(fetchMock, '/api/romeka/imports/6/preview')).toHaveLength(0);
    // The draft's two: the status line's counts, and the first look's question whether its files were ever looked at.
    expect(requestsFor(fetchMock, `${PATH}/preview`)).toHaveLength(2);
    expect(screen.queryByText('Saved')).not.toBeInTheDocument();
  });

  it('looks at the files once when a source was never looked at, and says so meanwhile', async () => {
    const full: Record<string, unknown> = { ...fixture('preview-check.json'), flags: fixture('preview-flags.json')['flags'] };
    let looked = false;
    let release = (): void => undefined;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const unread: Handler = (_url, init) => {
      const include = asRecord(JSON.parse(typeof init?.body === 'string' ? init.body : '{}'), [])['include'];
      const sections = Array.isArray(include) ? include.map(String) : [];
      const sources = Array.isArray(full['sources']) ? full['sources'] : [];
      return jsonResponse({
        ...Object.fromEntries(Object.entries(full).filter(([key]) => sections.includes(key))),
        pattern: full['pattern'],
        sources: looked ? sources : sources.map((source) => ({ ...asRecord(source, []), scanned: null, files: 0 })),
      });
    };
    const fetchMock = installFetch(
      route({
        [PATH]: () => jsonResponse(fixture('import-draft.json')),
        [`${PATH}/preview`]: unread,
        // The scan's answer arrives only when the test lets it, so the wait is seen.
        [`${PATH}/scan`]: () =>
          new Response(
            new ReadableStream<Uint8Array>({
              start: (controller) => {
                void gate.then(() => {
                  looked = true;
                  controller.enqueue(new TextEncoder().encode(JSON.stringify(fixture('scan.json'))));
                  controller.close();
                });
              },
            }),
            { headers: { 'Content-Type': 'application/json' } },
          ),
      }),
    );
    render(<ImportPage param="1" rest={['1', 'simple', 'found']} />);
    expect(await screen.findByText('Looking at your files for the first time...')).toBeInTheDocument();
    release();
    await waitFor(() => {
      expect(screen.queryByText('Looking at your files for the first time...')).toBeNull();
    });
    expect(requestsFor(fetchMock, `${PATH}/scan`)).toHaveLength(1);
  });

  it('does not look at files already looked at', async () => {
    const fetchMock = installFetch(
      route({ [PATH]: () => jsonResponse(fixture('import-draft.json')), [`${PATH}/preview`]: preview }),
    );
    render(<ImportPage param="1" rest={['1', 'simple', 'found']} />);
    await screen.findByText(CaseFold.label);
    await act(async () => {});
    expect(requestsFor(fetchMock, `${PATH}/scan`)).toHaveLength(0);
  });

  it('says so for a screen that does not exist and for an id that is not a number', async () => {
    installFetch(draftRoutes(() => jsonResponse({})));
    const { unmount } = render(at(['simple', 'nowhere']));
    expect(await screen.findByRole('alert')).toHaveTextContent('This import has no page with that name.');
    expect(screen.getByRole('link', { name: 'Open the import from the start' })).toHaveAttribute('href', '#/import/1');
    unmount();
    render(<ImportPage param="x1" rest={['x1']} />);
    expect(screen.getByRole('alert')).toHaveTextContent('"x1" is not the number of an import.');
  });
});

describe('Simple and Advanced', () => {
  it('lands on the matching screen and keeps the answer given in the other mode', async () => {
    let revision = 0;
    const fetchMock = installFetch(
      draftRoutes((init) => {
        revision += 1;
        return savedAs(init, revision);
      }),
    );
    const view = render(at(['simple', 'found']));
    const box = await screen.findByRole('checkbox', { name: CaseFold.label });
    const mode = screen.getByRole('group', { name: 'Mode' });
    expect(within(mode).getByRole('link', { name: 'Simple' })).toHaveAttribute('aria-current', 'page');
    expect(within(mode).getByRole('link', { name: 'Advanced' })).toHaveAttribute('href', '#/import/1/advanced/structure');
    fireEvent.click(box);
    // The screen may render the box again after the click, so the test reads the box it shows now.
    await waitFor(() => {
      expect(screen.getByRole('checkbox', { name: CaseFold.label })).toBeChecked();
    });
    expect(await screen.findByText(/^(Saving\.\.\.|Saved)$/)).toBeInTheDocument();

    view.rerender(at(['advanced', 'structure']));
    expect(await screen.findByText('caseFold: true')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Structure: each folder with its rules' })).toBeInTheDocument();
    const index = screen.getByRole('navigation', { name: 'Settings' });
    expect(within(index).getByRole('link', { name: 'Structure' })).toHaveAttribute('aria-current', 'page');
    expect(within(screen.getByRole('group', { name: 'Mode' })).getByRole('link', { name: 'Simple' })).toHaveAttribute(
      'href',
      '#/import/1/simple/structure',
    );
    expect(await screen.findByText('Saved')).toBeInTheDocument();

    view.rerender(at(['simple', 'found']));
    expect(screen.getByRole('checkbox', { name: CaseFold.label })).toBeChecked();
    const reads = requestsFor(fetchMock, PATH).filter((request) => request.init?.method !== 'PUT');
    expect(reads).toHaveLength(1);
    const puts = requestsFor(fetchMock, PATH).filter((request) => request.init?.method === 'PUT');
    expect(puts.map((request) => requestBody(request)['revision'])).toEqual([0]);
  });
});

describe('saving', () => {
  it('shows a reload prompt when someone else saved first, and loading it shows their answers', async () => {
    let theirs = false;
    const fetchMock = installFetch(
      route({
        [PATH]: (_url, init) => {
          if (init?.method === 'PUT') {
            theirs = true;
            return jsonResponse(fixture('conflict-stale.json'), { status: 409 });
          }
          return jsonResponse({ ...fixture('import-draft.json'), revision: theirs ? 1 : 0 });
        },
        [`${PATH}/preview`]: preview,
      }),
    );
    render(at(['simple', 'found']));
    const box = await screen.findByRole('checkbox', { name: CaseFold.label });
    fireEvent.click(box);
    const prompt = await screen.findByRole('alert', {}, { timeout: SAVE_DELAY_MS + 3000 });
    expect(prompt).toHaveTextContent('This import was saved somewhere else after you opened it');
    expect(screen.getByText('Not saved')).toBeInTheDocument();
    expect(box).toBeDisabled();
    fireEvent.click(within(prompt).getByRole('button', { name: 'Load the saved answers' }));
    await waitFor(() => {
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    });
    expect(screen.getByRole('checkbox', { name: CaseFold.label })).not.toBeChecked();
    expect(screen.getByText('Saved')).toBeInTheDocument();
    expect(requestsFor(fetchMock, PATH).filter((request) => request.init?.method !== 'PUT')).toHaveLength(2);
  });

  it('sends a waiting answer when the person leaves the import', async () => {
    const fetchMock = installFetch(draftRoutes((init) => savedAs(init, 1)));
    const view = render(at(['simple', 'found']));
    fireEvent.click(await screen.findByRole('checkbox', { name: CaseFold.label }));
    view.unmount();
    await act(async () => {});
    const puts = requestsFor(fetchMock, PATH).filter((request) => request.init?.method === 'PUT');
    expect(puts).toHaveLength(1);
  });
});
