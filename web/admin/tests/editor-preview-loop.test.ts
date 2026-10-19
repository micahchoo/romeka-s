/**
 * The live preview's request loop (`usePreview`): what it sends, which answer
 * it keeps, and when it does not ask again.
 */
import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { PageDraft } from '../src/pages/sites/editor/draft';
import { PREVIEW_DELAY_MS, usePreview } from '../src/pages/sites/editor/shell/usePreview';
import { apply, at, keyOf, load, parsePage, rawBlock } from './editor-draft-helpers';

const STORED = parsePage([]);
const SITE = STORED.site.id;
const PAGE = STORED.id;

interface Pending {
  readonly body: Record<string, unknown>;
  readonly signal: AbortSignal | null | undefined;
  readonly answer: (html: string) => void;
}

/** A fetch whose answers the test hands out, in any order, ignoring aborts. */
const manualFetch = (): Pending[] => {
  const pending: Pending[] = [];
  vi.stubGlobal(
    'fetch',
    vi.fn<typeof fetch>(
      (_input, init) =>
        new Promise<Response>((resolve) => {
          const text = typeof init?.body === 'string' ? init.body : '{}';
          const body: unknown = JSON.parse(text);
          pending.push({
            body: typeof body === 'object' && body !== null ? { ...body } : {},
            signal: init?.signal,
            answer: (html) => {
              resolve(new Response(html, { status: 200, headers: { 'Content-Type': 'text/html' } }));
            },
          });
        }),
    ),
  );
  return pending;
};

const start = (): PageDraft =>
  load([
    rawBlock('html', { html: '<p>One</p>' }),
    rawBlock('blockGroup', { span: '1' }),
    rawBlock('lineBreak', { break_type: 'opaque' }),
  ]);

const typed = (draft: PageDraft, html: string): PageDraft =>
  apply(draft, { type: 'setField', blockKey: keyOf(draft, 'html'), field: 'html', value: html });

const mount = (draft: PageDraft) =>
  renderHook(({ current }: { current: PageDraft }) => usePreview(current, 'my-site', SITE, PAGE), {
    initialProps: { current: draft },
  });

describe('usePreview', () => {
  it('asks for the first render at once and pairs the html with the block order it sent', async () => {
    const pending = manualFetch();
    const draft = start();
    const { result } = mount(draft);

    await waitFor(() => {
      expect(pending).toHaveLength(1);
    });
    const blocks = at(pending, 0).body['o:block'];
    expect(Array.isArray(blocks) ? blocks.length : 0).toBe(3);
    await act(async () => {
      at(pending, 0).answer('<p>first</p>');
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(result.current.status).toBe('ready');
    });
    expect(result.current.render).toEqual({
      html: '<p>first</p>',
      order: draft.blocks.map((block) => block.key),
    });
  });

  it('aborts the request in flight and never lets a late answer replace a newer one', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const pending = manualFetch();
    const draft = start();
    const { result, rerender } = mount(draft);
    await vi.waitFor(() => {
      expect(pending).toHaveLength(1);
    });

    rerender({ current: typed(draft, '<p>Two</p>') });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(PREVIEW_DELAY_MS);
    });
    expect(pending).toHaveLength(2);
    expect(at(pending, 0).signal?.aborted).toBe(true);

    await act(async () => {
      at(pending, 1).answer('<p>newer</p>');
      await Promise.resolve();
    });
    await vi.waitFor(() => {
      expect(result.current.render?.html).toBe('<p>newer</p>');
    });
    await act(async () => {
      at(pending, 0).answer('<p>older</p>');
      await vi.advanceTimersByTimeAsync(10);
    });
    expect(result.current.render?.html).toBe('<p>newer</p>');
    vi.useRealTimers();
  });

  it('does not send a body equal to the last one sent', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const pending = manualFetch();
    const draft = start();
    const { rerender } = mount(draft);
    await vi.waitFor(() => {
      expect(pending).toHaveLength(1);
    });

    // Typing and undoing inside the delay leaves the body the server already has.
    const edited = typed(draft, '<p>Two</p>');
    rerender({ current: edited });
    rerender({ current: apply(edited, { type: 'undo' }) });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(PREVIEW_DELAY_MS * 2);
    });
    expect(pending).toHaveLength(1);
    vi.useRealTimers();
  });
});
