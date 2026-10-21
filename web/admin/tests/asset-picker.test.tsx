/**
 * Site assets of any kind, through the pages a builder uses: the asset block
 * and the gallery take the file kinds their field declares, the upload asks for
 * a description a content slot can use, a background asks for none, and the
 * asset table and one asset's page name the file's kind instead of drawing it
 * as a picture it is not.
 */
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { useState } from 'react';
import type { ReactElement } from 'react';
import type { Mock } from 'vitest';
import { describe, expect, it } from 'vitest';
import { isRecord } from '../src/api/parse';
import { BLOCK_REGISTRY } from '../src/pages/sites/blockRegistry';
import type { PageDraft } from '../src/pages/sites/editor/draft';
import { draftReducer } from '../src/pages/sites/editor/reducer';
import { apply, blockAt, load } from './editor-draft-helpers';
import { RomkaForm } from '../src/pages/sites/editor/fields/RomkaForm';
import { romkaFor } from '../src/pages/sites/editor/romeka';
import { AssetsPage } from '../src/pages/assets/page';
import { BlockFormHarness, SITE_ID } from './editor-fields-helpers';
import { loadFixture } from './loadFixture';
import {
  installFetch,
  jsonResponse,
  listResponse,
  requestsFor,
  type CapturedRequest,
  type Handler,
} from './stub';

const PDF = {
  '@context': 'http://localhost/api-context',
  '@id': 'http://localhost/api/assets/9',
  '@type': ['o:Asset'],
  'o:id': 9,
  'o:owner': null,
  'o:name': 'doc.pdf',
  'o:filename': '9f8e7d6c5b4a39281706f5e4d3c2b1a0.pdf',
  'o:media_type': 'application/pdf',
  'o:asset_url': 'http://localhost/files/asset/9f8e7d6c5b4a39281706f5e4d3c2b1a0.pdf',
  'o:alt_text': null,
};

const IMAGE = {
  ...PDF,
  '@id': 'http://localhost/api/assets/11',
  'o:id': 11,
  'o:name': 'plan.png',
  'o:filename': '11223344556677889900aabbccddeeff.png',
  'o:media_type': 'image/png',
  'o:asset_url': 'http://localhost/files/asset/11223344556677889900aabbccddeeff.png',
};

const assetsPage = (extra: readonly unknown[] = []): readonly unknown[] => {
  const page = loadFixture('assets-page.json');
  if (!Array.isArray(page)) {
    throw new Error('expected an asset list fixture');
  }
  return [...page.filter(isRecord), ...extra];
};

const sitePages = (): readonly unknown[] => [
  loadFixture('site-page-1.json'),
  loadFixture('site-page-2-private.json'),
];

const assetHandler = (created: unknown = PDF): Handler => (url, init) => {
  if (url.pathname === '/api/assets' && init?.method === 'POST') {
    return jsonResponse(created);
  }
  if (url.pathname === '/api/assets/9') {
    return jsonResponse(PDF);
  }
  if (url.pathname === '/api/assets') {
    return listResponse(assetsPage([PDF]), 3);
  }
  if (url.pathname === '/api/users') {
    return jsonResponse(loadFixture('users-page.json'));
  }
  if (url.pathname === '/api/site_pages') {
    return jsonResponse(sitePages());
  }
  return jsonResponse({ errors: { error: `Unexpected request to ${url.pathname}` } }, { status: 404 });
};

const sent = (
  fetchMock: Mock<typeof fetch>,
  pathname: string,
  method: string,
): CapturedRequest | undefined =>
  requestsFor(fetchMock, pathname).find((request) => request.init?.method === method);

const entry = (asset = ''): Readonly<Record<string, unknown>> => ({
  id: asset,
  page: '',
  alt_link_title: '',
  caption: '',
});

const renderAssetBlock = (data: unknown = [entry()]): void => {
  render(<BlockFormHarness layout="asset" data={data} />);
};

/** A Romeka block (the gallery, the hero) is edited through `RomkaForm` over a draft. */
const RomkaHarness = ({ layout }: { readonly layout: string }): ReactElement => {
  const [state, setState] = useState<PageDraft>(() =>
    apply(load([]), { type: 'addBlock', layout, index: 0 }),
  );
  const block = blockAt(state.blocks, 0);
  const romka = romkaFor(BLOCK_REGISTRY, block);
  if (romka === undefined) {
    throw new Error(`the ${layout} carrier was not recognised`);
  }
  return (
    <RomkaForm
      block={block}
      romka={romka}
      issues={[]}
      siteId={SITE_ID}
      dispatch={(action) => {
        setState((current) => draftReducer(current, action));
      }}
    />
  );
};

/** Switch the gallery to assets and add one image row, then return its upload input. */
const galleryUpload = async (): Promise<HTMLElement> => {
  const source = await screen.findByLabelText('Images from');
  fireEvent.change(source, { target: { value: 'assets' } });
  if (screen.queryAllByLabelText('Upload').length === 0) {
    fireEvent.click(screen.getByRole('button', { name: 'Add item' }));
  }
  return screen.findByLabelText('Upload');
};

const uploadPdf = (input: HTMLElement): void => {
  fireEvent.change(input, {
    target: { files: [new File(['%PDF-1.4'], 'doc.pdf', { type: 'application/pdf' })] },
  });
};

describe('the asset picker takes the kinds its field declares', () => {
  it('offers only images in an image slot and any file in the asset block', async () => {
    installFetch(assetHandler());
    render(<RomkaHarness layout="gallery" />);

    expect(await galleryUpload()).toHaveAttribute('accept', 'image/*');
  });

  it('offers any file to the asset block, which has no accept set', async () => {
    installFetch(assetHandler());
    renderAssetBlock();

    expect(await screen.findByLabelText('Upload')).not.toHaveAttribute('accept');
  });

  it('names the asset block slot for files, not images', async () => {
    installFetch(assetHandler());
    renderAssetBlock();

    const files = await screen.findByRole('group', { name: 'Files' });
    expect(within(files).getByRole('group', { name: 'File' })).toBeInTheDocument();
    expect(
      within(files).getByText(
        'An uploaded file: an image shows inline, any other file shows as a link.',
      ),
    ).toBeInTheDocument();
  });

  it('refuses a dropped text file in an image slot before it sends anything', async () => {
    const fetchMock = installFetch(assetHandler());
    render(<RomkaHarness layout="gallery" />);
    const input = await galleryUpload();

    fireEvent.change(input, {
      target: { files: [new File(['notes'], 'notes.txt', { type: 'text/plain' })] },
    });

    expect(await screen.findByRole('alert')).toHaveTextContent(
      'This field takes an image. notes.txt is not one.',
    );
    expect(sent(fetchMock, '/api/assets', 'POST')).toBeUndefined();
  });
});

describe('the asset upload describes what it stored', () => {
  it('focuses the description after an upload and saves it as the alt text', async () => {
    const fetchMock = installFetch(assetHandler());
    renderAssetBlock();
    uploadPdf(await screen.findByLabelText('Upload'));

    const description = await waitFor(() => {
      const box = document.querySelector<HTMLTextAreaElement>('.asset-describe textarea');
      if (box === null) {
        throw new Error('expected the description field');
      }
      return box;
    });
    expect(description).toHaveFocus();
    fireEvent.change(description, { target: { value: 'The exhibition catalogue' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save description' }));

    const put = await waitFor(() => {
      const request = sent(fetchMock, '/api/assets/9', 'PUT');
      if (request === undefined) {
        throw new Error('expected the description save');
      }
      return request;
    });
    expect(put.init?.body).toBe(
      JSON.stringify({ 'o:name': 'doc.pdf', 'o:alt_text': 'The exhibition catalogue' }),
    );
    expect(await screen.findByText('Saved.')).toBeInTheDocument();
  });

  it('asks a background slot for no description', async () => {
    const fetchMock = installFetch(assetHandler(IMAGE));
    render(<RomkaHarness layout="hero" />);
    const input = await screen.findByLabelText('Upload');
    expect(input).toHaveAttribute('accept', 'image/*');
    fireEvent.change(input, {
      target: { files: [new File(['png'], 'plan.png', { type: 'image/png' })] },
    });

    await waitFor(() => {
      expect(sent(fetchMock, '/api/assets', 'POST')).toBeDefined();
    });
    expect(screen.queryByText('Save description')).toBeNull();
    expect(screen.queryByLabelText('Describe it for people who cannot see it')).toBeNull();
  });
});

describe('an asset that is not a picture', () => {
  it('shows the kind in the picker, never a broken image', async () => {
    installFetch(assetHandler());
    renderAssetBlock([entry('9')]);

    expect(await screen.findByText('PDF')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Choose asset' }));

    const tile = await screen.findByRole('button', { name: /doc\.pdf/ });
    expect(within(tile).getByText('PDF')).toBeInTheDocument();
    expect(within(tile).queryByRole('img')).toBeNull();
  });

  it('names the kind in the asset table, below the upload', async () => {
    installFetch(assetHandler());
    render(<AssetsPage />);

    const upload = await screen.findByLabelText('Upload');
    const table = await screen.findByRole('table');
    expect(upload.compareDocumentPosition(table) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(within(table).getByRole('columnheader', { name: 'Kind' })).toBeInTheDocument();
    const row = await screen.findByRole('row', { name: /doc\.pdf/ });
    expect(within(row).getAllByText('PDF').length).toBeGreaterThan(0);
  });

  it('previews the kind and names its description as link text', async () => {
    installFetch(assetHandler());
    render(<AssetsPage param="9" />);

    expect(await screen.findByRole('heading', { name: 'doc.pdf' })).toBeInTheDocument();
    expect(screen.queryByRole('img')).toBeNull();
    expect(screen.getByText('PDF')).toBeInTheDocument();
    expect(screen.getByText('Link text')).toBeInTheDocument();
    expect(screen.getByText('File').nextElementSibling).toHaveTextContent('.pdf');
  });
});
