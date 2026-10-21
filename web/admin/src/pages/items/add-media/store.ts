/**
 * The rows behind the add-media panel: one queue of bytes per item, and one
 * media create per file that arrived whole. The store outlives the panel (the
 * panel is inline and unmounts when it closes), so a file that is still
 * sending keeps sending while the curator works elsewhere in the admin, and a
 * media created after the panel closed still reaches the item page's grid.
 *
 * The bytes are `UploadSession`'s; this store decides what a whole file
 * becomes: it refuses what the server would refuse before sending anything,
 * hands a folder or a long list to Import, creates the media, and keeps the
 * description field's text.
 */
import { api } from '../../../api/client';
import { StagedUploads, createMedia } from '../../../api/staged-uploads';
import type { LimitsState } from '../../../api/staged-uploads';
import type { Item, ItemId, Media, MediaWrite, ValueWrite, Value } from '../../../api/types';
import { UploadSession } from '../../../app/upload/session';
import type { UploadSnapshot } from '../../../app/upload/session';
import type { Picked, PickedFile } from '../../../app/upload/pick';
import type { UploadTarget } from '../../../app/upload/target';

/** The most files one panel sends: a folder, a zip or more go to Import. */
export const MAX_FILES = 20;

export type SaveState =
  | { readonly kind: 'saving' }
  | { readonly kind: 'saved' }
  | { readonly kind: 'failed'; readonly reason: string };

export type AddState =
  | { readonly kind: 'adding' }
  | { readonly kind: 'added'; readonly media: Media; readonly sentAlt: string }
  | { readonly kind: 'failed'; readonly reason: string };

export type RefusedReason =
  | { readonly kind: 'size'; readonly limit: number }
  | { readonly kind: 'extension'; readonly extension: string }
  | { readonly kind: 'noExtension' };

export interface RefusedFile {
  readonly key: string;
  readonly name: string;
  readonly size: number;
  readonly reason: RefusedReason;
}

export type HandOff =
  | { readonly kind: 'many'; readonly count: number }
  | { readonly kind: 'folder' }
  | { readonly kind: 'zip' };

/** One link or HTML snippet being created. It has no bytes to send. */
export interface LinkRow {
  readonly key: string;
  readonly label: string;
  readonly state: AddState;
}

export interface AddsSnapshot {
  readonly uploads: UploadSnapshot;
  readonly limits: LimitsState | { readonly kind: 'loading' };
  readonly refused: readonly RefusedFile[];
  readonly handOff: HandOff | null;
  /** By upload row key. */
  readonly adds: ReadonlyMap<string, AddState>;
  /** The description typed per row, by upload row key. */
  readonly alt: ReadonlyMap<string, string>;
  /** How the last description save went, by upload row key. */
  readonly saved: ReadonlyMap<string, SaveState>;
  readonly links: readonly LinkRow[];
  readonly isPublic: boolean;
}

/** The value writes a full media PUT must carry, from the media the create answered. */
const valueWrites = (values: ReadonlyMap<string, readonly Value[]>): readonly ValueWrite[] =>
  [...values].flatMap(([term, list]) =>
    list.flatMap((value): ValueWrite[] => {
      switch (value.kind) {
        case 'literal':
          return [{ term, propertyId: value.propertyId, kind: 'literal', type: value.type, text: value.text, lang: value.lang, isPublic: value.isPublic }];
        case 'uri':
          return [{ term, propertyId: value.propertyId, kind: 'uri', type: value.type, uri: value.uri, label: value.label, lang: value.lang, isPublic: value.isPublic }];
        case 'resource':
          return [{ term, propertyId: value.propertyId, kind: 'resource', type: value.type, resourceId: value.resourceId, isPublic: value.isPublic }];
        case 'other':
          // A module data type this build cannot re-encode. A media created by
          // the panel carries none, so this only guards another caller.
          throw new Error('a value of a module data type cannot be written back');
      }
    }),
  );

/** The full `PUT /api/media/{id}` body: the row as it stands, with a new description. */
export const mediaWriteOf = (media: Media, altText: string | null): MediaWrite => ({
  isPublic: media.isPublic,
  owner: media.owner?.id ?? null,
  resourceClass: media.resourceClass?.id ?? null,
  resourceTemplate: media.resourceTemplate?.id ?? null,
  thumbnail: media.thumbnail?.id ?? null,
  values: valueWrites(media.values),
  lang: media.lang,
  altText,
});

const reasonOf = (error: unknown): string => (error instanceof Error ? error.message : String(error));

const extensionOf = (path: string): string | null => {
  const base = path.slice(path.lastIndexOf('/') + 1);
  const dot = base.lastIndexOf('.');
  if (dot <= 0 || dot === base.length - 1) {
    return null;
  }
  return base.slice(dot + 1).toLowerCase();
};

const nullIfBlank = (text: string | undefined): string | null => {
  const trimmed = text?.trim() ?? '';
  return trimmed === '' ? null : trimmed;
};

/** Where a link row's source lives, so a failed create can try again unchanged. */
export interface LinkSource {
  readonly kind: 'link' | 'html';
  readonly ingester: 'url' | 'youtube' | 'oembed' | 'iiif' | 'iiif_presentation' | 'html';
  readonly text: string;
}

export class AddStore {
  readonly itemId: ItemId;
  private item: Item;
  private readonly staged: StagedUploads;
  private readonly session: UploadSession;
  private readonly listeners = new Set<() => void>();
  private readonly addedListeners = new Set<(media: Media) => void>();
  private readonly adds = new Map<string, AddState>();
  private readonly alt = new Map<string, string>();
  private readonly saved = new Map<string, SaveState>();
  private readonly starting = new Set<string>();
  private readonly linkSources = new Map<string, LinkSource>();
  private refused: RefusedFile[] = [];
  private links: LinkRow[] = [];
  private handOff: HandOff | null = null;
  private limits: LimitsState | { readonly kind: 'loading' } = { kind: 'loading' };
  private isPublic: boolean;
  private asked = false;
  private refusedSeq = 0;
  private linkSeq = 0;
  private state: AddsSnapshot;

  constructor(item: Item, target?: UploadTarget) {
    this.item = item;
    this.itemId = item.id;
    this.isPublic = item.isPublic;
    this.staged = new StagedUploads(item.id);
    this.session = new UploadSession({
      target: target ?? this.staged,
      onFilesChanged: () => undefined,
    });
    this.state = this.build();
    this.session.subscribe(() => {
      this.created();
      this.publish();
    });
    void this.session.load();
    void this.staged.readLimits().then((limits) => {
      this.limits = limits;
      this.publish();
    });
  }

  /** The item the panel shows; its visibility is the default a new media takes. */
  setItem(item: Item): void {
    this.item = item;
    if (!this.asked) {
      this.isPublic = item.isPublic;
    }
  }

  get itemTitle(): string | null {
    return this.item.title;
  }

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  /** Every media this store created, for the item page's grid. */
  readonly subscribeAdded = (listener: (media: Media) => void): (() => void) => {
    this.addedListeners.add(listener);
    return () => {
      this.addedListeners.delete(listener);
    };
  };

  readonly snapshot = (): AddsSnapshot => this.state;

  /** True while any file is sending, being added, or waiting on a retry. */
  busy(): boolean {
    const uploads = this.session.snapshot().rows.some(
      (row) => row.phase.kind === 'sending' || row.phase.kind === 'waiting' || row.phase.kind === 'unpacking',
    );
    const adding = [...this.adds.values()].some((state) => state.kind === 'adding') || this.links.some((row) => row.state.kind === 'adding');
    return uploads || adding;
  }

  setPublic(isPublic: boolean): void {
    this.asked = true;
    this.isPublic = isPublic;
    this.publish();
  }

  /**
   * Picks or drops become rows: a folder, a zip or more than `MAX_FILES` goes
   * to Import; a file outside the allow list or over the size is refused here,
   * before any request; the rest join the queue.
   */
  addPicked(picked: Picked): void {
    this.handOff = null;
    const files = picked.files;
    if (files.some((file) => file.path.split('/').length > 1)) {
      this.handOff = { kind: 'folder' };
      this.publish();
      return;
    }
    if (files.some((file) => file.path.toLowerCase().endsWith('.zip'))) {
      this.handOff = { kind: 'zip' };
      this.publish();
      return;
    }
    if (files.length > MAX_FILES) {
      this.handOff = { kind: 'many', count: files.length };
      this.publish();
      return;
    }
    const limits = this.limits;
    const accepted: PickedFile[] = [];
    if (limits.kind === 'ready') {
      const allowed = new Set(limits.limits.extensions.map((extension) => extension.toLowerCase()));
      for (const entry of files) {
        const extension = extensionOf(entry.path);
        if (entry.file.size > limits.limits.maxFileBytes) {
          this.refuse(entry.path, entry.file.size, { kind: 'size', limit: limits.limits.maxFileBytes });
        } else if (!limits.limits.validationOff && extension === null) {
          this.refuse(entry.path, entry.file.size, { kind: 'noExtension' });
        } else if (!limits.limits.validationOff && !allowed.has(extension ?? '')) {
          this.refuse(entry.path, entry.file.size, { kind: 'extension', extension: (extension ?? '').toUpperCase() });
        } else {
          accepted.push(entry);
        }
      }
    } else {
      accepted.push(...files);
    }
    for (const entry of accepted) {
      const before = this.alt.get(entry.path);
      this.alt.set(entry.path, before ?? '');
    }
    this.publish();
    void this.session.add({ files: accepted, skipped: picked.skipped });
  }

  /** One link or HTML snippet, created without bytes. */
  addLink(source: LinkSource & { readonly label: string }): void {
    this.linkSeq += 1;
    const key = `link:${String(this.linkSeq)}`;
    this.linkSources.set(key, source);
    this.links = [...this.links, { key, label: source.label, state: { kind: 'adding' } }];
    this.publish();
    void this.createLink(key);
  }

  retryLink(key: string): void {
    const source = this.linkSources.get(key);
    if (source === undefined) {
      return;
    }
    this.setLink(key, { kind: 'adding' });
    void this.createLink(key);
  }

  /** A media create that failed: retry it against the staged bytes, or resend them. */
  retry(key: string): void {
    const row = this.session.snapshot().rows.find((candidate) => candidate.key === key);
    this.adds.delete(key);
    if (row?.phase.kind === 'failed') {
      this.session.retry(key);
    } else {
      this.created();
    }
    this.publish();
  }

  pause(key: string): void {
    this.session.pause(key);
  }

  resume(key: string): void {
    this.session.resume(key);
  }

  /** Removes the row: its staged bytes from the server, and any description kept for it. */
  discard(key: string): void {
    const link = this.linkSources.get(key);
    if (link !== undefined) {
      this.linkSources.delete(key);
      this.links = this.links.filter((row) => row.key !== key);
      this.publish();
      return;
    }
    this.adds.delete(key);
    this.alt.delete(key);
    this.saved.delete(key);
    this.publish();
    void this.session.remove(key);
  }

  removeRefused(key: string): void {
    this.refused = this.refused.filter((row) => row.key !== key);
    this.publish();
  }

  setAlt(key: string, text: string): void {
    this.alt.set(key, text);
    this.publish();
  }

  /** Saves the description on blur, once the media exists. */
  async saveAlt(key: string): Promise<void> {
    const state = this.adds.get(key);
    if (state?.kind !== 'added') {
      return;
    }
    const text = nullIfBlank(this.alt.get(key));
    if (text === state.sentAlt || (text === null && state.sentAlt === '')) {
      return;
    }
    this.saved.set(key, { kind: 'saving' });
    this.publish();
    try {
      const media = await api.updateMedia(state.media.id, mediaWriteOf(state.media, text));
      this.adds.set(key, { kind: 'added', media, sentAlt: text ?? '' });
      this.saved.set(key, { kind: 'saved' });
    } catch (error) {
      this.saved.set(key, { kind: 'failed', reason: reasonOf(error) });
    }
    this.publish();
  }

  private build(): AddsSnapshot {
    return {
      uploads: this.session.snapshot(),
      limits: this.limits,
      refused: this.refused,
      handOff: this.handOff,
      adds: new Map(this.adds),
      alt: new Map(this.alt),
      saved: new Map(this.saved),
      links: this.links,
      isPublic: this.isPublic,
    };
  }

  private publish(): void {
    this.state = this.build();
    for (const listener of this.listeners) {
      listener();
    }
  }

  /** Starts the create for every row whose bytes are whole and have no media yet. */
  private created(): void {
    if (!this.session.snapshot().loaded) {
      return;
    }
    for (const row of this.session.snapshot().rows) {
      if (row.phase.kind !== 'done' || this.adds.has(row.key) || this.starting.has(row.key)) {
        continue;
      }
      const upload = this.session.uploadOf(row.key);
      if (upload === null) {
        continue;
      }
      this.starting.add(row.key);
      this.adds.set(row.key, { kind: 'adding' });
      this.publish();
      // The text as it was when the create left, so a later edit is saved on blur.
      const sentAlt = nullIfBlank(this.alt.get(row.key));
      void createMedia(this.itemId, { kind: 'upload', upload }, { isPublic: this.isPublic, altText: sentAlt })
        .then((media) => {
          this.adds.set(row.key, { kind: 'added', media, sentAlt: sentAlt ?? '' });
          this.publish();
          for (const listener of this.addedListeners) {
            listener(media);
          }
        })
        .catch((error: unknown) => {
          this.adds.set(row.key, { kind: 'failed', reason: reasonOf(error) });
          this.publish();
        })
        .finally(() => {
          this.starting.delete(row.key);
        });
    }
  }

  private async createLink(key: string): Promise<void> {
    const source = this.linkSources.get(key);
    if (source === undefined) {
      return;
    }
    try {
      const media = await createMedia(
        this.itemId,
        source.kind === 'html'
          ? { kind: 'html', html: source.text }
          : { kind: 'link', ingester: source.ingester, url: source.text },
        { isPublic: this.isPublic, altText: null },
      );
      this.setLink(key, { kind: 'added', media, sentAlt: '' });
      for (const listener of this.addedListeners) {
        listener(media);
      }
    } catch (error) {
      this.setLink(key, { kind: 'failed', reason: reasonOf(error) });
    }
  }

  private setLink(key: string, state: AddState): void {
    this.links = this.links.map((row) => (row.key === key ? { ...row, state } : row));
    this.publish();
  }

  private refuse(name: string, size: number, reason: RefusedReason): void {
    this.refusedSeq += 1;
    this.refused = [...this.refused, { key: `refused:${String(this.refusedSeq)}`, name, size, reason }];
  }
}

/** One store per item, so a panel reopened on the same page finds its rows. */
const stores = new Map<ItemId, AddStore>();

export const addsFor = (item: Item, fresh?: UploadTarget): AddStore => {
  if (fresh !== undefined) {
    const store = new AddStore(item, fresh);
    stores.set(item.id, store);
    return store;
  }
  const known = stores.get(item.id);
  if (known !== undefined) {
    known.setItem(item);
    return known;
  }
  const store = new AddStore(item);
  stores.set(item.id, store);
  return store;
};

/** Forgets an item's queue, for an item deleted on another page or a test that starts over. */
export const forgetAdds = (itemId: ItemId): void => {
  stores.delete(itemId);
};
