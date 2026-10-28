/**
 * The fixity check: start one, read its counts, and the Media list filter each count opens
 * (`crates/omeka-server/src/fixity.rs`, `crates/omeka-api/src/fixity/view.rs`).
 *
 * The server counts the media the caller may see, through the same read as `GET /api/media`
 * with `romeka_fixity`, so each count equals the total of the list it opens.
 */
import { api } from './client';
import { asCount, member, nullable, strict } from './import-read';
import { asIdNumber, asString, brandId } from './parse';
import type { Id, Parser, Path } from './types';

/** The `job.class` of the check. */
export const FIXITY_JOB_CLASS = 'Romeka\\Job\\FixityCheck';

/** What the filter shows: every media the check read, or those of one outcome. */
export const FIXITY_SHOWN = ['checked', 'ok', 'differs', 'missing', 'unreadable'] as const;
export type FixityShown = (typeof FIXITY_SHOWN)[number];

export const isFixityShown = (value: string): value is FixityShown =>
  FIXITY_SHOWN.some((shown) => shown === value);

/** One check's counts, each the total of the Media list `fixityQuery` opens. */
export interface FixityCheck {
  readonly job: Id<'JobId'>;
  readonly status: string | null;
  readonly started: string;
  readonly ended: string | null;
  readonly checked: number;
  readonly ok: number;
  readonly differs: number;
  readonly missing: number;
  readonly unreadable: number;
}

/** The latest check with a result, and the check running now. */
export interface FixityLatest {
  readonly check: FixityCheck | null;
  readonly running: Id<'JobId'> | null;
}

const asJobId = (value: unknown, path: Path): Id<'JobId'> => brandId<'JobId'>(asIdNumber(value, path));

export const parseFixityCheck: Parser<FixityCheck> = (raw, path = []) => {
  const object = strict(raw, path, [
    'job',
    'status',
    'started',
    'ended',
    'checked',
    'ok',
    'differs',
    'missing',
    'unreadable',
  ]);
  return {
    job: member(object, 'job', path, asJobId),
    status: member(object, 'status', path, nullable(asString)),
    started: member(object, 'started', path, asString),
    ended: member(object, 'ended', path, nullable(asString)),
    checked: member(object, 'checked', path, asCount),
    ok: member(object, 'ok', path, asCount),
    differs: member(object, 'differs', path, asCount),
    missing: member(object, 'missing', path, asCount),
    unreadable: member(object, 'unreadable', path, asCount),
  };
};

export const parseFixityLatest: Parser<FixityLatest> = (raw, path = []) => {
  const object = strict(raw, path, ['check', 'running']);
  return {
    check: member(object, 'check', path, nullable(parseFixityCheck)),
    running: member(object, 'running', path, nullable(asJobId)),
  };
};

const parseStarted: Parser<Id<'JobId'>> = (raw, path = []) =>
  member(strict(raw, path, ['job']), 'job', path, asJobId);

/** Files that differ, are missing or cannot be read. */
export const problemsOf = (check: FixityCheck): number => check.differs + check.missing + check.unreadable;

/** The Media list one count opens. */
export const fixityHref = (job: Id<'JobId'>, shown: FixityShown): string =>
  `#/media?${new URLSearchParams({ fixity: shown, fixityJob: String(job) }).toString()}`;

/** The filter in a Media list hash, or `null` when it has none or a part of it does not parse. */
export const fixityFilterOf = (query: URLSearchParams): { readonly shown: FixityShown; readonly job: Id<'JobId'> } | null => {
  const shown = query.get('fixity');
  const job = Number(query.get('fixityJob'));
  if (shown === null || !isFixityShown(shown) || !Number.isSafeInteger(job) || job <= 0) {
    return null;
  }
  return { shown, job: brandId<'JobId'>(job) };
};

export const fixityApi = {
  /** The new check's job, or the one already running. */
  async start(): Promise<Id<'JobId'>> {
    return parseStarted(await api.writeJson('POST', '/api/romeka/fixity', {}));
  },
  async latest(): Promise<FixityLatest> {
    return parseFixityLatest(await api.requestJson('/api/romeka/fixity'));
  },
  async check(job: Id<'JobId'>): Promise<FixityCheck> {
    return parseFixityCheck(await api.requestJson(`/api/romeka/fixity/${String(job)}`));
  },
};
