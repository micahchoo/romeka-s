/**
 * A start, stop or undo the server refused with a 409, in plain words and with
 * the next step (`docs/import-v2.md`, "Wire details"). The table below is the
 * one place that maps a refusal to what the person sees and does next.
 */
import type { ReactElement } from 'react';
import type { ImportConflict } from '../../../api/import-conflict';
import type { ImportId } from '../../../api/import-plan';
import { placeHref } from '../screens';
import type { Mode } from '../screens';
import { useRunText } from './messages';
import type { RunMessageKey, RunTranslator } from './messages';
import { countWords } from './words';

/** What follows a refusal. The screen acts on `recount` and `refresh` by itself. */
export type NextStep =
  /** The counts are stale: count again, then the person adds. */
  | { readonly kind: 'recount' }
  /** A delete waits for its count to be typed. */
  | { readonly kind: 'confirm'; readonly deletes: number }
  /** Problems stop the start: the screen that lists them. */
  | { readonly kind: 'look' }
  /** Every item is left out: the screen that shows the files. */
  | { readonly kind: 'files' }
  | { readonly kind: 'open'; readonly importId: number }
  | { readonly kind: 'again' }
  /** The import is not where this page thought: read it again. */
  | { readonly kind: 'refresh' };

export interface Refusal {
  readonly says: string;
  readonly next: NextStep;
}

/**
 * The refusal of `conflict` for import `self`, or `null` for a code the run
 * routes never answer (a save's `stale`, `notDraft`, `v1Plan`, `typeExists`),
 * which the caller shows as a plain error.
 */
export const refusalOf = (conflict: ImportConflict, self: ImportId, translator: RunTranslator): Refusal | null => {
  const { t, number } = translator;
  const status = (key: RunMessageKey, value: Parameters<RunTranslator['t']>[0]): string => t(key, { status: t(value) });
  switch (conflict.code) {
    case 'changed':
      return { says: t('run.no.changed'), next: { kind: 'recount' } };
    case 'blocked':
      return {
        says: t('run.no.blocked', { what: countWords(translator, 'problems', conflict.blocking.length) }),
        next: { kind: 'look' },
      };
    case 'confirm':
      return {
        says: t('run.no.confirm', { count: number(conflict.deletes) }),
        next: { kind: 'confirm', deletes: conflict.deletes },
      };
    case 'busy':
      return conflict.importId === null || conflict.importId === self
        ? { says: t('run.no.busySelf'), next: { kind: 'again' } }
        : { says: t('run.no.busy'), next: { kind: 'open', importId: conflict.importId } };
    case 'nothing':
      return { says: t('run.no.nothing'), next: { kind: 'files' } };
    case 'cannotRun':
      return { says: status('run.no.cannotRun', `status.${conflict.status}`), next: { kind: 'refresh' } };
    case 'cannotUndo':
      return { says: status('run.no.cannotUndo', `status.${conflict.status}`), next: { kind: 'refresh' } };
    case 'notRunning':
      return { says: t('run.no.notRunning'), next: { kind: 'refresh' } };
    case 'stale':
    case 'notDraft':
    case 'v1Plan':
    case 'typeExists':
      return null;
  }
};

export const RefusalNotice = ({
  refusal,
  id,
  mode,
  onAgain,
}: {
  readonly refusal: Refusal;
  readonly id: ImportId;
  readonly mode: Mode;
  readonly onAgain: () => void;
}): ReactElement => {
  const { t } = useRunText();
  const { next } = refusal;
  return (
    <div role="alert" className="import-notice">
      <p>{refusal.says}</p>
      {next.kind === 'look' ? (
        <p>
          <a href={placeHref(id, mode === 'simple' ? { mode, screen: 'check' } : { mode, screen: 'problems' })}>{t('run.ready.look')}</a>
        </p>
      ) : null}
      {next.kind === 'files' ? (
        <p>
          <a href={placeHref(id, mode === 'simple' ? { mode, screen: 'found' } : { mode, screen: 'files' })}>{t('run.no.nothingLink')}</a>
        </p>
      ) : null}
      {next.kind === 'open' ? (
        <p>
          <a href={`#/import/${String(next.importId)}`}>{t('run.no.busyOpen')}</a>
        </p>
      ) : null}
      {next.kind === 'again' || next.kind === 'open' ? (
        <p>
          <button type="button" onClick={onAgain}>
            {t('run.no.again')}
          </button>
        </p>
      ) : null}
    </div>
  );
};
