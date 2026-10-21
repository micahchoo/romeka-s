/**
 * A media of the site as a block setting (`mediaRef`): the IIIF image's site
 * media source and the hero's background, with the hero's focal point set on
 * the chosen image.
 */
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { useReducer } from 'react';
import type { ReactElement } from 'react';
import { describe, expect, it } from 'vitest';
import { isRecord } from '../src/api/parse';
import { BLOCK_REGISTRY } from '../src/pages/sites/blockRegistry';
import type { BlockDraft, PageDraft } from '../src/pages/sites/editor/draft';
import { RomkaForm } from '../src/pages/sites/editor/fields/RomkaForm';
import { draftReducer } from '../src/pages/sites/editor/reducer';
import { carrierOf } from '../src/pages/sites/editor/romeka';
import { at, load, rawBlock } from './editor-draft-helpers';
import { BlockFormHarness, SITE_ID } from './editor-fields-helpers';
import { loadFixture } from './loadFixture';
import { installFetch, jsonResponse, listResponse, urlsFor } from './stub';

const item = loadFixture('item-1.json');
const media = loadFixture('media-951.json');
const withVisibility = (record: unknown, isPublic: boolean): unknown =>
  typeof record === 'object' && record !== null ? { ...record, 'o:is_public': isPublic } : record;
const fixtureMedia = (isPublic: boolean): unknown => withVisibility(media, isPublic);

/** The archive: item 1, whose one image is media 951; each public or private. */
const archive =
  (isPublic = true, itemPublic = true) =>
  (url: URL): Response => {
    if (url.pathname === '/api/items') {
      return listResponse([withVisibility(item, itemPublic)], 1);
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
    // Item 1 in the picker; item 31 as media 951's own item.
    if (url.pathname === '/api/items/1' || url.pathname === '/api/items/31') {
      return jsonResponse(withVisibility(item, itemPublic));
    }
    return jsonResponse({ errors: { error: `Unexpected request to ${url.pathname}` } }, { status: 404 });
  };

/** The row of a fixture page whose id the path names, or `null` for another path. */
const rowAt = (rows: unknown, pathname: string): Record<string, unknown> | null => {
  const id = /^\/api\/media\/(\d+)$/.exec(pathname)?.[1];
  if (id === undefined) {
    return null;
  }
  const list = Array.isArray(rows) ? rows : [];
  return (
    list.find((row): row is Record<string, unknown> => isRecord(row) && String(row['o:id']) === id) ??
    null
  );
};

/** The archive with an item whose media are the kinds fixture's rows. */
const archiveOfKinds =
  (rows: unknown, total: number) =>
  (url: URL): Response => {
    if (url.pathname === '/api/items') {
      return listResponse([item], 1);
    }
    if (url.pathname === '/api/item_sets' || url.pathname === '/api/resource_classes') {
      return listResponse([], 0);
    }
    if (url.pathname === '/api/media') {
      return listResponse(rows, total);
    }
    if (url.pathname === '/api/items/1' || url.pathname === '/api/items/31') {
      return jsonResponse(withVisibility(item, true));
    }
    const row = rowAt(rows, url.pathname);
    if (row !== null) {
      return jsonResponse(row);
    }
    return jsonResponse({ errors: { error: `Unexpected request to ${url.pathname}` } }, { status: 404 });
  };

/** The kinds fixture's rows, or only the audio one. */
const kindRows = (onlyAudio = false): readonly unknown[] => {
  const rows: unknown = loadFixture('media-page-kinds.json');
  const list = Array.isArray(rows) ? rows : [];
  return onlyAudio ? list.filter((_, index) => index === 1) : list;
};

/** The kinds fixture's rows with its audio row read as a video, so a film is one of them. */
const rowsWithVideo = (): readonly unknown[] =>
  kindRows().map((row) =>
    isRecord(row) && row['o:media_type'] === 'audio/mpeg'
      ? {
          ...row,
          'o:media_type': 'video/mp4',
          'o:source': 'clip.mp4',
          'o:filename': 'clip.mp4',
          'o:original_url': 'http://localhost/files/original/clip.mp4',
        }
      : row,
  );

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

  it('does not let the curator choose a public media of a private item', async () => {
    installFetch(archive(true, false));
    const block = renderIiif();
    fireEvent.change(screen.getByLabelText('Image from'), { target: { value: 'media' } });
    fireEvent.click(await screen.findByRole('option', { name: 'Fixture Item One', selected: false }));
    expect(await screen.findByRole('radio', { name: /Private/ })).toBeDisabled();
    expect(carrierOf(block())?.settings).toEqual({ source: 'media' });
  });

  it('lists the media it cannot zoom disabled, with the reason', async () => {
    const fetchMock = installFetch(archiveOfKinds(kindRows(), kindRows().length));
    const block = renderIiif();
    fireEvent.change(screen.getByLabelText('Image from'), { target: { value: 'media' } });
    fireEvent.click(await screen.findByRole('option', { name: 'Fixture Item One', selected: false }));

    expect(await screen.findByRole('radio', { name: 'photo.jpg' })).toBeEnabled();
    // The reason is visible text in the tile, so a screen reader hears it too.
    expect(
      screen.getByRole('radio', { name: /tone\.mp3.*this archive cannot zoom it/ }),
    ).toBeDisabled();
    expect(
      screen.getByRole('radio', { name: /report\.pdf.*this archive cannot zoom it/ }),
    ).toBeDisabled();
    // The one image is public, so the control chooses it at once.
    expect(await screen.findByRole('radio', { checked: true })).toBeInTheDocument();
    expect(carrierOf(block())?.settings).toEqual({ source: 'media', media: 960 });
    // The field asks the archive only for items that hold an image.
    expect(
      urlsFor(fetchMock, '/api/items').map((url) => url.searchParams.getAll('has_media_kind')),
    ).toContainEqual(['local-image', 'iiif-image']);
  });

  it('keeps an item whose only media is an audio file listed, and refuses it', async () => {
    installFetch(archiveOfKinds(kindRows(true), 1));
    const block = renderIiif();
    fireEvent.change(screen.getByLabelText('Image from'), { target: { value: 'media' } });
    fireEvent.click(await screen.findByRole('option', { name: 'Fixture Item One', selected: false }));

    expect(await screen.findByRole('radio', { name: /tone\.mp3/ })).toBeDisabled();
    // Hiding it would say the item has no image, which reads as a fault.
    expect(screen.queryByText('This item has no image.')).toBeNull();
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

  it('keeps a chosen media that turned private and says visitors do not see it', async () => {
    installFetch(archive(false));
    render(<HeroForm initial={heroDraft({ background_source: 'media', background_media: 951 })} />);
    expect(await screen.findByText('This image is private, so visitors do not see it.')).toBeInTheDocument();
    expect(heroSettings()).toMatchObject({ background_media: 951 });
  });

  it('says nothing of privacy while the chosen media is public', async () => {
    installFetch(archive());
    render(<HeroForm initial={heroDraft({ background_source: 'media', background_media: 951 })} />);
    // The item's images are listed once its visibility is known.
    await waitFor(() => {
      expect(document.querySelector('.media-choice input[type="radio"]')).not.toBeNull();
    });
    expect(screen.queryByText('This image is private, so visitors do not see it.')).toBeNull();
  });

  it('says a chosen media with no picture cannot take a focal point', async () => {
    installFetch(archiveOfKinds(kindRows(true), 1));
    render(<HeroForm initial={heroDraft({ background_source: 'media', background_media: 961 })} />);

    expect(
      await screen.findByText('This media has no image to set a focal point on.'),
    ).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^Focal point/ })).toBeNull();
    expect(heroSettings()).toMatchObject({ background_media: 961 });
  });

  it('asks for an image first when the hero has none', () => {
    render(<HeroForm initial={heroDraft({})} />);
    expect(screen.getByText('Choose a background image first.')).toBeInTheDocument();
  });

  it("lists the item's image enabled and its video and PDF disabled with the reason", async () => {
    installFetch(archiveOfKinds(rowsWithVideo(), 3));
    render(<HeroForm initial={heroDraft({ background_source: 'media' })} />);
    fireEvent.click(await screen.findByRole('option', { name: 'Fixture Item One', selected: false }));

    // The item holds one image, so the control chooses it at once.
    await waitFor(() => {
      expect(heroSettings()).toMatchObject({ background_media: 960 });
    });
    fireEvent.click(screen.getByRole('button', { name: 'Choose another item' }));
    const choices = screen.getByRole('group', { name: 'Choose one of its images' });
    expect(await within(choices).findByRole('radio', { name: 'photo.jpg' })).toBeEnabled();
    expect(
      within(choices).getByRole('radio', { name: /clip\.mp4.*not an image/ }),
    ).toBeDisabled();
    expect(
      within(choices).getByRole('radio', { name: /report\.pdf.*not an image/ }),
    ).toBeDisabled();
    // One line counts what the item holds, so the greyed tiles explain themselves.
    expect(screen.getByText('Media in this item:')).toBeInTheDocument();
    expect(screen.getByText('1 Image')).toBeInTheDocument();
    expect(screen.getByText('1 Video')).toBeInTheDocument();
    expect(screen.getByText('1 PDF')).toBeInTheDocument();
  });

  it('keeps a stored video in a hero, says what visitors see, and saves it unchanged', async () => {
    installFetch(archiveOfKinds(rowsWithVideo(), 3));
    render(<HeroForm initial={heroDraft({ background_source: 'media', background_media: 961 })} />);

    expect(
      await screen.findByText(
        'Visitors see nothing here: this field takes images, and Video is not one.',
      ),
    ).toBeInTheDocument();
    expect(heroSettings()).toMatchObject({ background_media: 961 });
  });
});
