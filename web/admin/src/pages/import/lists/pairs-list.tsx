/**
 * Pair many rows in a spreadsheet (`docs/import-v2.md`, "Pairs list round
 * trip"): download the rows that name no file, write each row's file in the
 * Files column, send the list back. The server turns each Files cell that
 * names exactly one file into a `pair` decision and reports every other cell
 * with its reason, so nothing is guessed and nothing is lost silently.
 */
import { useId, useState } from 'react';
import type { ReactElement } from 'react';
import { importApi } from '../../../api/import-routes';
import type { Decision, ImportPlan } from '../../../api/import-plan';
import type { PairsAdded, PairsWhy } from '../../../api/import-view';
import { ErrorNotice } from '../../../app/errors';
import { toError } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { ListDrop } from './list-drop';
import { useListsText } from './messages';
import type { ListsMessageKey } from './messages';

/** Past this many rows, one at a time is slow (`docs/import-v2.md`, decision of 2026-10-20). */
export const PAIR_BY_HAND = 30;
const CELLS_SHOWN = 20;
/** The header the server writes, never translated: the person looks for these letters in the spreadsheet. */
const FILES_COLUMN = 'Files';

const WHY: Readonly<Record<PairsWhy, ListsMessageKey>> = {
  noRow: 'lists.pairs.why.noRow',
  noFile: 'lists.pairs.why.noFile',
  manyFiles: 'lists.pairs.why.manyFiles',
  noItem: 'lists.pairs.why.noItem',
};

type Sent =
  | { readonly at: 'idle' }
  | { readonly at: 'sending'; readonly name: string }
  | { readonly at: 'failed'; readonly error: Error }
  | { readonly at: 'done'; readonly result: PairsAdded; readonly made: readonly Decision[] };

const samePair = (a: Decision, b: Decision): boolean =>
  a.kind === 'pair' && b.kind === 'pair' && a.row.sheet === b.row.sheet && a.row.key === b.row.key && a.file.source === b.file.source && a.file.path === b.file.path;

/** The plan without the pairs a list added, and nothing else. */
export const withoutPairs = (plan: ImportPlan, made: readonly Decision[]): ImportPlan => ({
  ...plan,
  decisions: plan.decisions.filter((decision) => !made.some((one) => samePair(one, decision))),
});

const Result = ({ result, made, onUndo, editable }: { readonly result: PairsAdded; readonly made: readonly Decision[]; readonly onUndo: () => void; readonly editable: boolean }): ReactElement => {
  const { t, number } = useListsText();
  const shown = result.unmatched.slice(0, CELLS_SHOWN);
  return (
    <div className="import-note" role="status">
      <p>
        <b>{result.added === 0 ? t('lists.pairs.none') : result.added === 1 ? t('lists.pairs.addedOne') : t('lists.pairs.added', { count: number(result.added) })}</b>{' '}
        {result.already === 0 ? null : t('lists.pairs.already', { count: number(result.already) })}{' '}
        {result.unmatchedTotal === 0 ? null : t('lists.pairs.unmatched', { count: number(result.unmatchedTotal) })}
      </p>
      {result.unmatchedTotal === 0 ? null : (
        <details>
          <summary>{t('lists.pairs.show', { count: number(result.unmatchedTotal) })}</summary>
          <ul>
            {shown.map((cell, index) => (
              <li key={index}>
                {t('lists.pairs.cell', { row: cell.row, text: cell.text })} <span className="muted">{t(WHY[cell.why])}</span>
              </li>
            ))}
          </ul>
          {result.unmatchedTotal > shown.length ? <p className="muted">{t('lists.trip.andMore', { count: number(result.unmatchedTotal - shown.length) })}</p> : null}
        </details>
      )}
      {made.length === 0 ? null : (
        <button type="button" disabled={!editable} onClick={onUndo}>
          {t('lists.pairs.undo')}
        </button>
      )}
    </div>
  );
};

/** Offered above 30 rows that name no file; `rows` is all of them, however the lists below are searched. */
export const PairsList = ({ rows, example }: { readonly rows: number; readonly example: string | null }): ReactElement | null => {
  const { t, number } = useListsText();
  const draft = useDraft();
  const id = useId();
  const [sent, setSent] = useState<Sent>({ at: 'idle' });
  // Offered by the rows left, but kept while it holds a send: an upload that pairs the rows down to 30 or fewer must still show its result and its undo.
  if (rows <= PAIR_BY_HAND && sent.at === 'idle') {
    return null;
  }
  // An edit not yet saved would be lost under the revision the list makes.
  const canSend = draft.editable && draft.save.status === 'saved' && sent.at !== 'sending';

  const send = (file: File): void => {
    const before = draft.plan.decisions.length;
    setSent({ at: 'sending', name: file.name });
    importApi.addPairs(draft.id, file).then(
      (result) => {
        draft.adopt(result.import);
        setSent({ at: 'done', result, made: result.import.plan.decisions.slice(before).filter((decision) => decision.kind === 'pair') });
      },
      (error: unknown) => {
        setSent({ at: 'failed', error: toError(error) });
      },
    );
  };

  return (
    <section className="import-card import-trip" aria-labelledby={`${id}-title`}>
      <h4 id={`${id}-title`}>{t('lists.pairs.title', { count: number(rows) })}</h4>
      <p className="muted">{t('lists.pairs.lede', { column: FILES_COLUMN, example: example ?? t('lists.pairs.example') })}</p>
      <p>
        <a className="button-link" href={importApi.pairsUrl(draft.id)} download>
          {t('lists.pairs.download', { count: number(rows) })}
        </a>
      </p>
      <p className="import-note">{t('lists.pairs.exact', { column: FILES_COLUMN })}</p>
      <ListDrop canSend={canSend} waitingForSave={draft.editable && draft.save.status !== 'saved'} onFile={send} />
      {sent.at === 'sending' ? <p role="status">{t('lists.trip.sending', { file: sent.name })}</p> : null}
      {sent.at === 'failed' ? <ErrorNotice error={sent.error} /> : null}
      {sent.at === 'done' ? (
        <Result
          result={sent.result}
          made={sent.made}
          editable={draft.editable}
          onUndo={() => {
            const made = sent.made;
            draft.change((plan) => withoutPairs(plan, made));
            setSent({ at: 'idle' });
          }}
        />
      ) : null}
    </section>
  );
};
