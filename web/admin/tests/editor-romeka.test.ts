/**
 * The Romeka carrier in the draft model: the picker's block becomes an upstream
 * carrier, its settings edit through the reducer, and a stored plain rendering
 * that no longer matches its hash is reported so the editor can mark the block.
 */
import { describe, expect, it } from 'vitest';
import generated from '../src/generated/blocks.json';
import {
  draftReducer,
  toApi,
  validate,
} from '../src/pages/sites/editor/draft';
import type { PageDraft } from '../src/pages/sites/editor/draft';
import {
  carrierOf,
  plainHash,
  romkaState,
  settingsOf,
} from '../src/pages/sites/editor/romeka';
import { apply, at, blockAt, load, rawBlock } from './editor-draft-helpers';

const carrierBlock = (plain: string, plainHashValue: string | null): unknown =>
  rawBlock('html', { html: plain }, {
    'o:layout_data': {
      romeka: {
        block: 'hero',
        v: 1,
        settings: { heading: 'Welcome', overlay: 'medium', alignment: 'left' },
        ...(plainHashValue === null ? {} : { plain_hash: plainHashValue }),
      },
    },
  });

describe('the plain hash', () => {
  it('matches the generator sample', () => {
    expect(plainHash(generated.romekaHash.input)).toBe(generated.romekaHash.hash);
    expect(generated.romekaHash.algorithm).toBe('fnv1a-64');
  });
});

describe('adding a Romeka block', () => {
  it('stores the registered carrier layout and its settings defaults', () => {
    const draft = apply(load([]), { type: 'addBlock', layout: 'hero', index: 0 });
    const block = blockAt(draft.blocks, 0);
    expect(block.layout).toBe('html');
    expect(block.data).toEqual({ html: '' });
    const carrier = carrierOf(block);
    expect(carrier).not.toBeNull();
    expect(carrier?.block).toBe('hero');
    expect(settingsOf(block)).toEqual({ overlay: 'medium', alignment: 'left' });
  });

  it('offers every generated field through the carrier schema', () => {
    const hero = generated.romeka.find((entry) => entry.name === 'hero');
    expect(hero?.carrier).toBe('html');
    expect(hero?.fields.map((field) => field.key)).toEqual([
      'heading',
      'subheading',
      'body',
      'background',
      'overlay',
      'alignment',
      'cta_label',
      'cta_page',
      'cta_url',
    ]);
  });
});

describe('editing a Romeka block', () => {
  const withSetting = (state: PageDraft, value: string): PageDraft =>
    apply(state, {
      type: 'setRomkaSetting',
      blockKey: blockAt(state.blocks, 0).key,
      field: 'heading',
      value,
    });

  it('writes a setting into the carrier and keeps the other keys', () => {
    const draft = withSetting(load([carrierBlock('<h2>Old</h2>', null)]), 'New heading');
    const block = blockAt(draft.blocks, 0);
    expect(settingsOf(block).heading).toBe('New heading');
    const writes = toApi(draft);
    expect(at(writes, 0).layout).toBe('html');
    expect(at(writes, 0).layoutData).toEqual({
      romeka: {
        block: 'hero',
        v: 1,
        settings: { heading: 'New heading', overlay: 'medium', alignment: 'left' },
      },
    });
  });

  it('reports the required heading as an issue on the block', () => {
    const draft = load([carrierBlock('<h2>Old</h2>', null)]);
    const cleared = apply(draft, {
      type: 'setRomkaSetting',
      blockKey: blockAt(draft.blocks, 0).key,
      field: 'heading',
      value: '',
    });
    expect(validate(cleared)).toContainEqual({
      kind: 'required',
      blockKey: blockAt(draft.blocks, 0).key,
      field: 'heading',
    });
  });
});

describe('a carrier edited outside Romeka', () => {
  it('is edited when the stored html does not match the hash', () => {
    const draft = load([carrierBlock('<p>changed</p>', plainHash('<h2>Welcome</h2>'))]);
    expect(romkaState(draft.registry, blockAt(draft.blocks, 0))).toBe('edited');
  });

  it('is ok when the stored html matches the hash', () => {
    const plain = '<h2>Welcome</h2>';
    const draft = load([carrierBlock(plain, plainHash(plain))]);
    expect(romkaState(draft.registry, blockAt(draft.blocks, 0))).toBe('ok');
  });

  it('clears the hash when the Romeka version is chosen, keeping the html', () => {
    const draft = load([carrierBlock('<p>changed</p>', plainHash('<h2>Welcome</h2>'))]);
    const key = blockAt(draft.blocks, 0).key;
    const restored = apply(draft, { type: 'useRomkaVersion', blockKey: key });
    const block = blockAt(restored.blocks, 0);
    expect(block.data).toEqual({ html: '<p>changed</p>' });
    expect(carrierOf(block)?.plainHash).toBeNull();
    expect(romkaState(restored.registry, block)).toBe('unhashed');
    expect(
      draftReducer(restored, { type: 'undo' }).blocks.length,
    ).toBe(1);
  });

  it('keeps an unknown key beside the carrier', () => {
    const draft = load([
      rawBlock('html', { html: '<p>x</p>' }, {
        'o:layout_data': {
          class: 'keep',
          romeka: { block: 'hero', v: 1, settings: { heading: 'H' }, extra: true },
        },
      }),
    ]);
    const restored = apply(draft, {
      type: 'useRomkaVersion',
      blockKey: blockAt(draft.blocks, 0).key,
    });
    const layoutData = at(toApi(restored), 0).layoutData;
    expect(layoutData).toEqual({
      class: 'keep',
      romeka: { block: 'hero', v: 1, settings: { heading: 'H' }, extra: true },
    });
  });
});
