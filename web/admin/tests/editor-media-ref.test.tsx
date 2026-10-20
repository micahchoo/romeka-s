/**
 * A media of the site as a block setting (`mediaRef`): the IIIF image's site
 * media source and the hero's background, with the hero's focal point set on
 * the chosen image.
 */
import { act, fireEvent, render, screen } from '@testing-library/react';
import { useReducer } from 'react';
import type { ReactElement } from 'react';
import { describe, expect, it } from 'vitest';
import { BLOCK_REGISTRY } from '../src/pages/sites/blockRegistry';
import type { BlockDraft, PageDraft } from '../src/pages/sites/editor/draft';
import { RomkaForm } from '../src/pages/sites/editor/fields/RomkaForm';
import { draftReducer } from '../src/pages/sites/editor/reducer';
import { carrierOf } from '../src/pages/sites/editor/romeka';
import { at, load, rawBlock } from './editor-draft-helpers';
import { BlockFormHarness, SITE_ID } from './editor-fields-helpers';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse } from './stub';

const item = loadFixture('item-1.json');
const media = loadFixture('media-951.json');
const fixtureMedia = (isPublic: boolean): unknown =>
  typeof media === 'object' && media !== null ? { ...media, 'o:is_public': isPublic } : media;

/** The archive: item 1, whose one image is media 951, public or private. */
const archive =
  (isPublic = true) =>
  (url: URL): Response => {
    if (url.pathname === '/api/items') {
      return listResponse([item], 1);
    }
    if (url.pathname === '/api/item_sets' || url.pathname === '/api/resource_classes') {
      return listResponse([], 0);
    }
    if (url.pathname === '/api/media') {
      return listResponse([fixtureMedia(isPublic)], 1);
    }
    if (url.pathname === '/api/media/951') {
      return jsonResponse(fixtureMedia(isPublic));
    }
    if (url.pathname === '/api/items/1') {
      return jsonResponse(item);
    }
    return jsonResponse({ errors: { error: `Unexpected request to ${url.pathname}` } }, { status: 404 });
  };

const renderIiif = (): (() => BlockDraft) => {
  let latest: BlockDraft | null = null;
  render(
    <BlockFormHarness
      layout="iiifImage"
      data={{ image_url: '' }}
      onBlock={(block) => {
        latest = block;
      }}
    />,
  );
  return () => {
    if (latest === null) {
      throw new Error('the form rendered no block');
    }
    return latest;
  };
};

describe('the IIIF image from a media of this site', () => {
  it('stores the chosen media and the source as Romeka settings, leaving o:data alone', async () => {
    installFetch(archive());
    const block = renderIiif();
    expect(screen.getByRole('textbox', { name: 'Image URL' })).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('Image from'), { target: { value: 'media' } });
    expect(screen.queryByRole('textbox', { name: 'Image URL' })).toBeNull();
    fireEvent.click(await screen.findByRole('option', { name: 'Fixture Item One', selected: false }));
    // The item has one public image, so the control chooses it at once.
    expect(await screen.findByRole('radio', { checked: true })).toBeInTheDocument();
    expect(carrierOf(block())?.settings).toEqual({ source: 'media', media: 951 });
    expect(block().data).toEqual({ image_url: '' });
    // With a media chosen, the archive folds away until the curator asks for it.
    expect(screen.queryByRole('listbox', { name: 'Find the item' })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Choose another item' }));
    expect(await screen.findByRole('listbox', { name: 'Find the item' })).toBeInTheDocument();
  });

  it('shows a private media but does not let the curator choose it', async () => {
    installFetch(archive(false));
    const block = renderIiif();
    fireEvent.change(screen.getByLabelText('Image from'), { target: { value: 'media' } });
    fireEvent.click(await screen.findByRole('option', { name: 'Fixture Item One', selected: false }));
    const radio = await screen.findByRole('radio', { name: /Private/ });
    expect(radio).toBeDisabled();
    expect(carrierOf(block())?.settings).toEqual({ source: 'media' });
  });
});

const hero = BLOCK_REGISTRY.romeka.find((entry) => entry.name === 'hero');

const HeroForm = ({ initial }: { readonly initial: PageDraft }): ReactElement => {
  const [draft, dispatch] = useReducer(draftReducer, initial);
  if (hero === undefined) {
    throw new Error('the registry has no hero');
  }
  latestHero = at(draft.blocks, 0);
  return <RomkaForm block={latestHero} romka={hero} issues={[]} siteId={SITE_ID} dispatch={dispatch} />;
};

let latestHero: BlockDraft | null = null;

const heroSettings = (): Readonly<Record<string, unknown>> => {
  if (latestHero === null) {
    throw new Error('the hero form rendered no block');
  }
  return carrierOf(latestHero)?.settings ?? {};
};

const heroDraft = (settings: Readonly<Record<string, unknown>>): PageDraft =>
  load([
    rawBlock('html', { html: '' }, {
      'o:layout_data': { romeka: { block: 'hero', v: 2, settings: { heading: 'Maps', ...settings } } },
    }),
  ]);

describe('the hero background from a media of this site', () => {
  it('swaps the asset picker for the media control when the source changes', () => {
    installFetch(archive());
    render(<HeroForm initial={heroDraft({})} />);
    expect(screen.getByText('Background image')).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('Background from'), { target: { value: 'media' } });
    expect(screen.queryByText('Background image')).toBeNull();
    expect(screen.getByText('Background media')).toBeInTheDocument();
    expect(heroSettings()).toMatchObject({ background_source: 'media' });
  });

  it('sets the focal point on the chosen media with the arrow keys, and centers it again', async () => {
    installFetch(archive());
    render(<HeroForm initial={heroDraft({ background_source: 'media', background_media: 951 })} />);
    const frame = await screen.findByRole('button', { name: /^Focal point: 50% across, 50% down/ });
    expect(frame.querySelector('img')).toHaveAttribute(
      'src',
      'http://localhost:8124/files/original/e82cef00da75e9c5661a9ee66155ef419cf89988.jpg',
    );
    fireEvent.keyDown(frame, { key: 'ArrowRight' });
    fireEvent.keyDown(screen.getByRole('button', { name: /^Focal point/ }), { key: 'ArrowUp' });
    expect(heroSettings()).toMatchObject({ focal_x: 55, focal_y: 45 });
    fireEvent.click(screen.getByRole('button', { name: 'Center' }));
    expect(heroSettings()).toMatchObject({ focal_x: null, focal_y: null });
  });

  it('places the focal point where the curator clicks', async () => {
    installFetch(archive());
    render(<HeroForm initial={heroDraft({ background_source: 'media', background_media: 951 })} />);
    const frame = await screen.findByRole('button', { name: /^Focal point/ });
    frame.getBoundingClientRect = () => DOMRect.fromRect({ x: 100, y: 50, width: 200, height: 100 });
    act(() => {
      fireEvent.click(frame, { clientX: 150, clientY: 125 });
    });
    expect(heroSettings()).toMatchObject({ focal_x: 25, focal_y: 75 });
  });

  it('asks for an image first when the hero has none', () => {
    render(<HeroForm initial={heroDraft({})} />);
    expect(screen.getByText('Choose a background image first.')).toBeInTheDocument();
  });
});
