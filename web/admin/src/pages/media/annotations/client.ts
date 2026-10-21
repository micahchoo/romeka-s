/**
 * The admin's only fetch for media notes and captions: the WAP container and
 * its annotations (contract section 6), plus the caption track routes A-2
 * proposes (G3, G4, G5).
 *
 * Every response is parsed at this boundary; nothing here returns `unknown`.
 * A 412 carries the server's current note in `NoteConflict`; a 404 carries
 * `NoteGone`. The cookie rides along, so the server's session decides the
 * rights.
 */
import { ApiError, defaultBaseUrl } from '../../../api/client';
import {
  asBoolean,
  asNumber,
  asRecord,
  asString,
  field,
  parseApiErrors,
  parseListOf,
} from '../../../api/parse';
import type { ApiErrors, MediaId, Path } from '../../../api/types';
import { localizeServerErrors } from '../../../i18n/serverText';
import { fromW3C, notesOf, parseSurface, targetSourceOf, toW3C, trackIdsOf } from './wire';
import type { Container, Draft, Note } from './wire';

const ANNO_PROFILE = 'application/ld+json; profile="http://www.w3.org/ns/anno.jsonld"';
const VTT = 'text/vtt';
const SRT = 'application/x-subrip';

/** What someone else did first: the server's version of the note (412). */
export class NoteConflict extends Error {
  readonly current: Note;

  constructor(current: Note) {
    super('HTTP 412');
    this.name = 'NoteConflict';
    this.current = current;
  }
}

/** The note is no longer there (404): someone deleted it. */
export class NoteGone extends Error {
  constructor() {
    super('HTTP 404');
    this.name = 'NoteGone';
  }
}

/** One caption language, from `GET …/tracks` (G3). */
export interface TrackSummary {
  readonly lang: string;
  readonly cues: number;
  readonly imported: string | null;
  readonly handEdited: number;
}

/** The answer to an import (contract 6.5). */
export interface TrackImport {
  readonly lang: string | null;
  readonly cues: number;
  readonly replaced: boolean;
}

/** What a dry run would change (G5); times are milliseconds. */
export interface TrackPreview {
  readonly cues: number;
  readonly startMs: number;
  readonly endMs: number;
  readonly replaces: number;
  readonly handEdited: number;
}

export interface NotesClientOptions {
  readonly baseUrl?: string;
  readonly fetchFn?: typeof fetch;
}

const readBody = async (response: Response): Promise<unknown> => {
  const text = await response.text();
  if (text === '') {
    return null;
  }
  const value: unknown = JSON.parse(text);
  return value;
};

const firstMessage = (errors: ApiErrors | null): string | null => {
  if (errors === null) {
    return null;
  }
  for (const messages of Object.values(errors)) {
    const first = messages[0];
    if (first !== undefined) {
      return first;
    }
  }
  return null;
};

/** The server's error document, in the user's language, as the API client does it. */
const refusal = async (response: Response): Promise<ApiError> => {
  const body = await readBody(response).catch(() => null);
  const parsed = body === null ? null : parseApiErrors(body);
  const errors = parsed === null ? null : await localizeServerErrors(parsed);
  const message = firstMessage(errors) ?? `HTTP ${String(response.status)}`;
  return new ApiError(response.status, errors, message, parsed, body);
};

/** A note built from an answer that carried its ETag. */
const noteOf = (response: Response, body: unknown, trackIds: ReadonlySet<string>): Note => ({
  ...fromW3C(body, trackIds),
  etag: response.headers.get('ETag') ?? '',
});

/** A 412: the note in the body is the conflicting version, with its own ETag. */
const conflictOf = async (response: Response): Promise<Error> => {
  const body = await readBody(response).catch(() => null);
  if (body === null) {
    return new ApiError(response.status, null, 'HTTP 412', null, null);
  }
  return new NoteConflict(noteOf(response, body, new Set()));
};

const captionType = (file: Blob): string => {
  const name = 'name' in file && typeof file.name === 'string' ? file.name.toLowerCase() : '';
  if (name.endsWith('.vtt')) {
    return VTT;
  }
  if (name.endsWith('.srt')) {
    return SRT;
  }
  if (file.type === VTT || file.type === SRT) {
    return file.type;
  }
  return file.type === '' ? VTT : file.type;
};

const parseTrackSummary = (raw: unknown, path: Path): TrackSummary => {
  const track = asRecord(raw, path);
  return {
    lang: asString(field(track, 'lang', path), [...path, 'lang']),
    cues: asNumber(field(track, 'cues', path), [...path, 'cues']),
    imported: typeof track['imported'] === 'string' ? track['imported'] : null,
    handEdited: asNumber(field(track, 'handEdited', path), [...path, 'handEdited']),
  };
};

const trackSummaries = parseListOf(parseTrackSummary);

const parseTrackImport = (raw: unknown): TrackImport => {
  const document = asRecord(raw, []);
  return {
    lang: typeof document['lang'] === 'string' ? document['lang'] : null,
    cues: asNumber(field(document, 'cues', []), ['cues']),
    replaced: asBoolean(field(document, 'replaced', []), ['replaced']),
  };
};

const parseTrackPreview = (raw: unknown): TrackPreview => {
  const document = asRecord(raw, []);
  return {
    cues: asNumber(field(document, 'cues', []), ['cues']),
    startMs: asNumber(field(document, 'start', []), ['start']),
    endMs: asNumber(field(document, 'end', []), ['end']),
    replaces: asNumber(field(document, 'replaces', []), ['replaces']),
    handEdited: asNumber(field(document, 'handEdited', []), ['handEdited']),
  };
};

const canvasOf = (notes: readonly Note[]): string => {
  for (const note of notes) {
    const canvas = targetSourceOf(note.raw);
    if (canvas !== null) {
      return canvas;
    }
  }
  return '';
};

export class NotesClient {
  private readonly baseUrl: string;
  private readonly fetchFn: typeof fetch;

  constructor(options: NotesClientOptions = {}) {
    this.baseUrl = options.baseUrl ?? defaultBaseUrl();
    this.fetchFn = options.fetchFn ?? ((input, init) => globalThis.fetch(input, init));
  }

  /**
   * Every note on the media, every page followed. The canvas comes from
   * `Romeka-Canvas`, or from a note's target when the server sends no header:
   * a create needs it, and an empty container has no other source.
   */
  async container(mediaId: MediaId): Promise<Container> {
    const url = this.url(`/annotations/media/${String(mediaId)}/`);
    const response = await this.fetchFn(url, {
      headers: { Accept: ANNO_PROFILE },
      credentials: 'same-origin',
    });
    if (!response.ok) {
      throw await refusal(response);
    }
    const surface = parseSurface(response.headers.get('Romeka-Surface'));
    const allow = response.headers.get('Allow') ?? '';
    const writable = allow.split(',').some((method) => method.trim().toUpperCase() === 'POST');
    const trackIds = new Set(trackIdsOf(response.headers.get('Romeka-Track-Ids')));
    const document = asRecord(await readBody(response), []);
    const notes: Note[] = [];
    let page = document['first'];
    while (page !== undefined && page !== null) {
      const pageDocument =
        typeof page === 'string' ? await this.pageDocument(page, trackIds) : asRecord(page, ['first']);
      notes.push(...notesOf(pageDocument, trackIds));
      page = typeof pageDocument['next'] === 'string' ? pageDocument['next'] : undefined;
    }
    const total = document['total'] === undefined ? notes.length : asNumber(document['total'], ['total']);
    return {
      surface,
      canvas: response.headers.get('Romeka-Canvas') ?? canvasOf(notes),
      writable,
      total,
      notes,
    };
  }

  /**
   * `canvas` is the media canvas IRI (the container's `canvas`). A retry of the
   * same draft with the same key is the server's replay: it answers 200 with
   * the same note instead of a second row.
   */
  async create(mediaId: MediaId, draft: Draft, key: string, canvas: string): Promise<Note> {
    const url = this.url(`/annotations/media/${String(mediaId)}/`);
    const response = await this.fetchFn(url, {
      method: 'POST',
      headers: { Accept: ANNO_PROFILE, 'Content-Type': ANNO_PROFILE, 'Idempotency-Key': key },
      body: JSON.stringify(toW3C(draft, canvas)),
      credentials: 'same-origin',
    });
    if (response.status === 404) {
      throw new NoteGone();
    }
    if (!response.ok) {
      throw await refusal(response);
    }
    return noteOf(response, await readBody(response), new Set());
  }

  /**
   * The whole annotation the server last returned, with the draft's fields
   * set. When the note came from a page (no revision), the ETag is read first,
   * because a page document carries none.
   */
  async replace(note: Note, draft: Draft): Promise<Note> {
    const canvas = targetSourceOf(note.raw);
    if (canvas === null) {
      throw new Error('the server document names no canvas');
    }
    let base = note.raw;
    let etag = note.etag;
    if (etag === '') {
      const current = await this.reread(note.iri);
      base = current.body;
      etag = current.etag;
    }
    if (etag === '') {
      throw new Error(`no ETag for ${note.iri}`);
    }
    const response = await this.fetchFn(note.iri, {
      method: 'PUT',
      headers: { Accept: ANNO_PROFILE, 'Content-Type': ANNO_PROFILE, 'If-Match': etag },
      body: JSON.stringify(toW3C(draft, canvas, base)),
      credentials: 'same-origin',
    });
    if (response.status === 412) {
      throw await conflictOf(response);
    }
    if (response.status === 404) {
      throw new NoteGone();
    }
    if (!response.ok) {
      throw await refusal(response);
    }
    return {
      ...noteOf(response, await readBody(response), new Set()),
      fromTrack: note.fromTrack,
    };
  }

  async remove(note: Note): Promise<void> {
    const etag = note.etag === '' ? (await this.reread(note.iri)).etag : note.etag;
    const response = await this.fetchFn(note.iri, {
      method: 'DELETE',
      headers: { Accept: ANNO_PROFILE, 'If-Match': etag },
      credentials: 'same-origin',
    });
    if (response.status === 412) {
      throw await conflictOf(response);
    }
    if (response.status === 404) {
      throw new NoteGone();
    }
    if (!response.ok) {
      throw await refusal(response);
    }
  }

  /**
   * One note with its current strong ETag. The panel reads the note an editor
   * opens, so a colleague who saves after that read moves the revision past
   * the one this editor holds: their next `PUT` or `DELETE` is refused with 412
   * rather than quietly overwriting the colleague.
   */
  async read(note: Note): Promise<Note> {
    const current = await this.reread(note.iri);
    return {
      ...fromW3C(current.body, new Set()),
      etag: current.etag,
      fromTrack: note.fromTrack,
    };
  }

  /** The caption languages of the media and their cue counts (G3). */
  async tracks(mediaId: MediaId): Promise<readonly TrackSummary[]> {
    const response = await this.fetchFn(this.url(`/annotations/media/${String(mediaId)}/tracks`), {
      headers: { Accept: 'application/json' },
      credentials: 'same-origin',
    });
    if (!response.ok) {
      throw await refusal(response);
    }
    return trackSummaries(await readBody(response));
  }

  /** Remove one language's imported cues; hand-written notes stay (G4). */
  async removeTrack(mediaId: MediaId, lang: string): Promise<void> {
    const query = new URLSearchParams({ lang });
    const response = await this.fetchFn(
      this.url(`/annotations/media/${String(mediaId)}/tracks?${query.toString()}`),
      { method: 'DELETE', credentials: 'same-origin' },
    );
    if (response.status === 404) {
      throw new NoteGone();
    }
    if (!response.ok) {
      throw await refusal(response);
    }
  }

  /** The import's effect, with no row changed (G5). */
  async previewTrack(mediaId: MediaId, file: Blob, lang: string | null): Promise<TrackPreview> {
    const query = new URLSearchParams({ dry_run: '1' });
    if (lang !== null) {
      query.set('lang', lang);
    }
    const response = await this.fetchFn(
      this.url(`/annotations/media/${String(mediaId)}/tracks?${query.toString()}`),
      {
        method: 'POST',
        headers: { Accept: 'application/json', 'Content-Type': captionType(file) },
        body: file,
        credentials: 'same-origin',
      },
    );
    if (!response.ok) {
      throw await refusal(response);
    }
    return parseTrackPreview(await readBody(response));
  }

  /** The file's bytes; the server is the only parser (contract 1.2). */
  async importTrack(mediaId: MediaId, file: Blob, lang: string | null): Promise<TrackImport> {
    const query = new URLSearchParams();
    if (lang !== null) {
      query.set('lang', lang);
    }
    const suffix = query.toString() === '' ? '' : `?${query.toString()}`;
    const response = await this.fetchFn(
      this.url(`/annotations/media/${String(mediaId)}/tracks${suffix}`),
      {
        method: 'POST',
        headers: { Accept: 'application/json', 'Content-Type': captionType(file) },
        body: file,
        credentials: 'same-origin',
      },
    );
    if (!response.ok) {
      throw await refusal(response);
    }
    return parseTrackImport(await readBody(response));
  }

  private async pageDocument(
    iri: string,
    trackIds: Set<string>,
  ): Promise<Record<string, unknown>> {
    const response = await this.fetchFn(iri, {
      headers: { Accept: ANNO_PROFILE },
      credentials: 'same-origin',
    });
    if (!response.ok) {
      throw await refusal(response);
    }
    for (const id of trackIdsOf(response.headers.get('Romeka-Track-Ids'))) {
      trackIds.add(id);
    }
    return asRecord(await readBody(response), []);
  }

  /** The current document and its ETag, for a note read from a page. */
  private async reread(iri: string): Promise<{ readonly etag: string; readonly body: unknown }> {
    const response = await this.fetchFn(iri, {
      headers: { Accept: ANNO_PROFILE },
      credentials: 'same-origin',
    });
    if (response.status === 404) {
      throw new NoteGone();
    }
    if (!response.ok) {
      throw await refusal(response);
    }
    return { etag: response.headers.get('ETag') ?? '', body: await readBody(response) };
  }

  private url(path: string): string {
    return `${this.baseUrl}${path}`;
  }
}

/** The application-wide client. Same origin, so the session cookie rides along. */
export const notesClient = new NotesClient();

/**
 * The captions file of one language, as the public page and a `<track>` read
 * it (contract 6.5). A plain link and a player's `src` both use it, so the
 * route is spelled here once.
 */
export const captionsUrl = (mediaId: MediaId, lang: string): string =>
  `${defaultBaseUrl()}/annotations/media/${String(mediaId)}/captions.vtt?lang=${encodeURIComponent(lang)}`;
