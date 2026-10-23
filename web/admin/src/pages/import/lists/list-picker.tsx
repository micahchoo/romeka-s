/**
 * Every list file the scan found, the one that pairs the most rows first,
 * each with the counts that say how well it fits the files. The person uses
 * any number of lists, turns one down and still sees the rest, and is asked
 * once when a list pairs the same items as one in use: two catalogues of one
 * item are allowed, and so is a newer one in place of an old copy.
 * `useListChoice` holds those edits for one list.
 *
 * The picker is of one folder. A list that another folder reads shows as
 * used there, and "Move here" takes it from that folder: it never offers
 * "Stop using", which would take it from a folder the person is not looking
 * at. Which lists a folder reads is an owned write (`advanced/writes.ts`):
 * each folder whose lists change is then written by hand, so the proposer
 * neither gives the list back nor reads it twice. On a suggested folder of
 * Advanced's Structure, a list the proposal offers that folder is a click
 * instead (`OfferedList`): Use and Stop using send the edit Simple sends.
 */
import { useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import type { ImportPlan, LevelId, ListFile, SheetPlan } from '../../../api/import-plan';
import type { ListCandidate } from '../../../api/import-sections';
import { ErrorNotice } from '../../../app/errors';
import { toError } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { declines, readState, withState } from '../simple/example-state';
import { listEdit } from '../simple/list-read';
import { useListsText } from './messages';
import { removeSheet } from './round-trip';
import { usePreview } from './use-preview';
import { allLists, seriesHolding } from '../series';
import { written } from '../advanced/writes';
import type { OfferedWriter } from '../advanced/writes';

/** Where a list stands for the series a picker is of. A list the series reads is in use even when its automatic read fails. */
export type ListState =
  | { readonly kind: 'inUse'; readonly sheet: SheetPlan }
  | { readonly kind: 'usedBy'; readonly sheet: SheetPlan; readonly series: LevelId }
  | { readonly kind: 'declined' }
  | { readonly kind: 'offered' }
  | { readonly kind: 'unreadable' };

const sheetFor = (plan: ImportPlan, source: string, file: string): SheetPlan | undefined =>
  allLists(plan).find((sheet) => sheet.source === source && sheet.file === file);

/** The plan names the folder that reads a list, as the scan's `ListCandidate.series` does, and stays current before the next scan answers. */
export const stateOf = (plan: ImportPlan, candidate: ListCandidate, level: LevelId): ListState => {
  const sheet = sheetFor(plan, candidate.source, candidate.file);
  const holder = sheet === undefined ? undefined : seriesHolding(plan, sheet.id);
  if (sheet !== undefined && holder !== undefined) {
    return holder.id === level ? { kind: 'inUse', sheet } : { kind: 'usedBy', sheet, series: holder.id };
  }
  if (candidate.read !== 'ok') {
    return { kind: 'unreadable' };
  }
  return readState(plan).notLists.some((entry) => declines(entry, candidate.source, candidate.file)) ? { kind: 'declined' } : { kind: 'offered' };
};

/** The list in use that pairs the most of the same items as `candidate`, which adding it asks about. */
const overlapInUse = (plan: ImportPlan, candidate: ListCandidate): { readonly sheet: SheetPlan; readonly shared: number } | null => {
  for (const overlap of candidate.overlaps) {
    const sheet = sheetFor(plan, overlap.source, overlap.file);
    if (sheet !== undefined) {
      return { sheet, shared: overlap.sharedTargets };
    }
  }
  return null;
};

const withDecline = (plan: ImportPlan, source: string, file: string): ImportPlan =>
  withState(plan, { notLists: [...readState(plan).notLists.filter((entry) => !declines(entry, source, file)), { source, file }] });

const withoutDecline = (plan: ImportPlan, source: string, file: string): ImportPlan =>
  withState(plan, { notLists: readState(plan).notLists.filter((entry) => !declines(entry, source, file)) });

export const nameOf = (file: string): { readonly name: string; readonly folder: string } => {
  const at = file.lastIndexOf('/');
  return at < 0 ? { name: file, folder: '' } : { name: file.slice(at + 1), folder: file.slice(0, at) };
};

const STATE_WORDS = {
  inUse: 'lists.picker.inUse',
  declined: 'lists.picker.declined',
  offered: 'lists.picker.offered',
  unreadable: 'lists.picker.unreadable',
} as const;

export const stateWords = (state: ListState, t: ReturnType<typeof useListsText>['t']): string =>
  state.kind === 'usedBy' ? t('lists.picker.usedBy', { folder: state.series }) : t(STATE_WORDS[state.kind]);

/** What the scan counted for one list, and where it stands. */
const Facts = ({ candidate, state }: { readonly candidate: ListCandidate; readonly state: ListState }): ReactElement => {
  const { t, number } = useListsText();
  const { best } = candidate;
  const { name, folder } = nameOf(candidate.file);
  const params = best === null ? null : { column: best.header, count: number(best.paired), rows: number(candidate.rows) };
  return (
    <>
      <p>
        <b>{name}</b> {folder === '' ? null : <span className="muted">{t('lists.picker.folder', { folder })} </span>}
        {candidate.read === 'ok' ? <>· {candidate.rows === 1 ? t('lists.picker.rowsOne') : t('lists.picker.rows', { count: number(candidate.rows) })} </> : null}
        · <b>{stateWords(state, t)}</b>
      </p>
      {candidate.read === 'ok' ? null : <p role="alert">{t('lists.picker.unread', { message: candidate.read.error })}</p>}
      {candidate.read === 'ok' && params === null ? <p className="muted">{t('lists.picker.none')}</p> : null}
      {best === null || params === null ? null : (
        <p>
          {best.on === 'fileName' ? t('lists.picker.byNames', params) : t('lists.picker.byCodes', params)}
          {best.pairedNormalized > best.paired ? <> {t('lists.picker.loose', { count: number(best.pairedNormalized - best.paired) })}</> : null}
        </p>
      )}
      {candidate.overlaps.map((overlap) => (
        <p key={`${overlap.source}/${overlap.file}`}>{t('lists.picker.sameItems', { file: overlap.file, count: number(overlap.sharedTargets) })}</p>
      ))}
    </>
  );
};

/** The one question before a list that pairs the same items as `other` joins it. */
const Ask = ({ other, shared, busy, answer }: { readonly other: SheetPlan; readonly shared: number; readonly busy: boolean; readonly answer: (replacing: SheetPlan | null | 'cancel') => void }): ReactElement => {
  const { t, number } = useListsText();
  return (
    <div className="guided-ask">
      <p className="guided-ask-q">{t('lists.picker.shared', { count: number(shared), file: other.file })}</p>
      <p className="import-row-actions">
        <button type="button" className="import-primary" disabled={busy} onClick={() => { answer(null); }}>
          {t('lists.picker.both')}
        </button>
        <button type="button" disabled={busy} onClick={() => { answer(other); }}>
          {t('lists.picker.instead', { file: other.file })}
        </button>
        <button type="button" disabled={busy} onClick={() => { answer('cancel'); }}>
          {t('lists.picker.cancel')}
        </button>
      </p>
    </div>
  );
};

type Step =
  | { readonly at: 'idle' }
  | { readonly at: 'asking'; readonly other: SheetPlan; readonly shared: number }
  | { readonly at: 'reading' }
  | { readonly at: 'failed'; readonly error: Error };

/**
 * The lists the proposal offers a suggested folder, and the click that picks
 * one (`null`: no list). `by` is the offered write it is (`advanced/writes.ts`).
 */
export interface OfferedList {
  readonly by: OfferedWriter;
  readonly lists: readonly ListFile[];
  readonly send: (list: ListFile | null) => void;
}

/** One list's place in the plan, and the edits that change it. */
export interface ListChoice {
  readonly state: ListState;
  readonly reading: boolean;
  readonly asking: boolean;
  readonly busy: boolean;
  /** Reads the list into the plan, asking first when it pairs the same items as a list in use. */
  readonly use: () => void;
  /** Takes the list from the folder that reads it into this one. */
  readonly move: () => void;
  /** Takes the list out, and keeps it out. */
  readonly stop: () => void;
  /** Says the list is not about these items. */
  readonly decline: () => void;
  /** The overlap question while it is asked, and the error of a read that failed. */
  readonly notice: ReactNode;
}

/** The list `candidate`, read into the series `level` when used, or picked by `offered`'s click when it offers it. */
export const useListChoice = (candidate: ListCandidate, level: LevelId, offered?: OfferedList): ListChoice => {
  const draft = useDraft();
  const [step, setStep] = useState<Step>({ at: 'idle' });
  const state = stateOf(draft.plan, candidate, level);
  const { source, file } = candidate;
  const offers = offered?.lists.some((one) => one.source === source && one.file === file) === true ? offered : undefined;
  const busy = !draft.editable || step.at === 'reading';

  const add = (replacing: SheetPlan | null): void => {
    setStep({ at: 'reading' });
    const before = replacing === null ? draft.plan : removeSheet(draft.plan, replacing.id);
    listEdit(draft.id, before, level, candidate)
      .then((edit) => {
        draft.change((plan) => {
          const freed = replacing === null ? plan : written(plan, 'list', seriesHolding(plan, replacing.id)?.id, (now) => removeSheet(now, replacing.id));
          return written(freed, 'list', level, (now) => edit(withoutDecline(now, source, file)));
        });
        setStep({ at: 'idle' });
      })
      .catch((error: unknown) => {
        setStep({ at: 'failed', error: toError(error) });
      });
  };
  const use = (): void => {
    if (offers !== undefined) {
      offers.send({ source, file });
      return;
    }
    const overlap = overlapInUse(draft.plan, candidate);
    if (overlap === null) {
      add(null);
    } else {
      setStep({ at: 'asking', other: overlap.sheet, shared: overlap.shared });
    }
  };
  const move = (): void => {
    if (state.kind === 'usedBy') {
      add(state.sheet);
    }
  };
  const stop = (): void => {
    if (state.kind === 'inUse' && offers !== undefined) {
      offers.send(null);
    } else if (state.kind === 'inUse') {
      const sheet = state.sheet;
      draft.change((plan) => written(plan, 'list', level, (now) => withDecline(removeSheet(now, sheet.id), source, file)));
    }
  };
  const decline = (): void => {
    draft.change((plan) => withDecline(plan, source, file));
  };

  let notice: ReactNode = null;
  if (step.at === 'asking') {
    notice = (
      <Ask
        other={step.other}
        shared={step.shared}
        busy={busy}
        answer={(replacing) => {
          if (replacing === 'cancel') {
            setStep({ at: 'idle' });
          } else {
            add(replacing);
          }
        }}
      />
    );
  } else if (step.at === 'failed') {
    notice = <ErrorNotice error={step.error} />;
  }
  return { state, reading: step.at === 'reading', asking: step.at === 'asking', busy, use, move, stop, decline, notice };
};

const Row = ({ candidate, level, offered }: { readonly candidate: ListCandidate; readonly level: LevelId; readonly offered: OfferedList | undefined }): ReactElement => {
  const { t } = useListsText();
  const choice = useListChoice(candidate, level, offered);
  const { state, busy } = choice;
  let actions: ReactNode = null;
  if (choice.asking) {
    actions = null;
  } else if (state.kind === 'inUse') {
    actions = (
      <p className="import-row-actions">
        <button type="button" disabled={busy} onClick={choice.stop}>
          {t('lists.picker.stop')}
        </button>
      </p>
    );
  } else if (state.kind === 'usedBy') {
    actions = (
      <p className="import-row-actions">
        <button type="button" disabled={busy} onClick={choice.move}>
          {choice.reading ? t('lists.picker.reading') : t('lists.picker.move')}
        </button>
      </p>
    );
  } else if (state.kind !== 'unreadable') {
    actions = (
      <p className="import-row-actions">
        <button type="button" className={state.kind === 'offered' ? 'import-primary' : undefined} disabled={busy} onClick={choice.use}>
          {choice.reading ? t('lists.picker.reading') : t('lists.picker.use')}
        </button>
        {state.kind === 'offered' ? (
          <button type="button" disabled={busy} onClick={choice.decline}>
            {t('lists.picker.decline')}
          </button>
        ) : null}
      </p>
    );
  }
  return (
    <li aria-label={candidate.file} data-decision="list" className="import-note">
      <Facts candidate={candidate} state={state} />
      {actions}
      {choice.notice}
    </li>
  );
};

/** The picker, which reads a list into the series `level`; `none` is what to show when the folders hold no list file. */
export const ListPicker = ({ none, level, offered }: { readonly none: ReactNode; readonly level: LevelId; readonly offered?: OfferedList }): ReactElement => {
  const { t } = useListsText();
  const read = usePreview(['listCandidates'], {});
  if (read.status === 'loading') {
    return <p role="status" className="import-slow">{t('shell.loading')}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const candidates = read.data.listCandidates;
  if (candidates.length === 0) {
    return <>{none}</>;
  }
  return (
    <section aria-label={t('lists.picker.title')} aria-busy={read.busy}>
      <h4>{t('lists.picker.title')}</h4>
      <p className="muted">{t('lists.picker.order')}</p>
      <ul className="import-pick">
        {candidates.map((candidate) => (
          <Row key={`${candidate.source}/${candidate.file}`} candidate={candidate} level={level} offered={offered} />
        ))}
      </ul>
    </section>
  );
};

export default ListPicker;
