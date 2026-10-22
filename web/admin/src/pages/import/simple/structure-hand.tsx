/**
 * A folder written by hand. Simple's hand edit is a marked example: the
 * person marks one name of the folder, reads what the folder makes with the
 * mark, and uses it. "Use this" writes the folder's series and adds its
 * `own` edit, so the server keeps the series as written and suggests nothing
 * more on it (design call 4); "Back to our suggestion" deletes that edit.
 *
 * The count before "Use this" and the count after are the same number: the
 * items the server's proposal counts for the written series (`OwnNode`).
 *
 * This is a lazy chunk of its own: the Structure screen loads it only when
 * the person marks an example or selects a folder written by hand.
 */
import { createContext, useContext, useState } from 'react';
import type { ReactElement } from 'react';
import { parseProposal } from '../../../api/import-proposal';
import type { CutCounts, OwnNode } from '../../../api/import-proposal';
import type { Edit, FolderScope, LevelId } from '../../../api/import-plan';
import { ErrorNotice } from '../../../app/errors';
import { toError } from '../../../app/useAsync';
import { withMarkOn } from '../example';
import { usePreview } from '../lists/use-preview';
import { withSeries } from '../series';
import { ShowUsOneExample } from './example';
import type { Marked } from './example';
import { QUESTIONS } from './structure-edits';
import type { Clicks, Write } from './structure-edits';
import { bucketsWords, itemsWords, keptWords, useStructureText } from './structure-lines';
import { ImportQuestion, useScreen } from './structure-parts';

/** The folder written by hand, and the clicks that write it. */
export interface Hand {
  readonly clicks: Clicks;
  readonly editable: boolean;
  readonly level: LevelId;
  readonly scope: FolderScope;
  /** Closes the marking: after "Use this", or on "Cancel". */
  readonly onDone: () => void;
}

const HandContext = createContext<Hand | null>(null);

const useHand = (): Hand => {
  const hand = useContext(HandContext);
  if (hand === null) {
    throw new Error('a mark is counted outside a folder written by hand');
  }
  return hand;
};

const ownOf = (scope: FolderScope): Edit => ({ edit: 'own', scope });

const writeMark = (level: LevelId, { mark, path }: Marked): Write => (plan) => withSeries(plan, level, (series) => withMarkOn(series, path, mark));

/** What the written series makes: its items, and its files per item when an item has more than one. */
const Makes = ({ items }: { readonly items: CutCounts }): ReactElement => {
  const { t, number } = useStructureText();
  const several = items.perItem.some((bucket) => bucket.files !== '1' && bucket.count > 0);
  return (
    <>
      <p role="status">{t('structure.hand.makes', { items: itemsWords(items.items, t, number) })}</p>
      {several ? <p>{t('structure.perItem', { buckets: bucketsWords(items.perItem, t, number) })}</p> : null}
    </>
  );
};

const ownNodeIn = (raw: unknown, level: LevelId): OwnNode | Error | undefined => {
  try {
    return parseProposal(raw).nodes.find((node): node is OwnNode & { kind: 'own' } => node.kind === 'own' && node.level === level);
  } catch (error: unknown) {
    return toError(error);
  }
};

/** The folder with the mark, counted by the server before it is used. */
const SeriesTrial = (marked: Marked): ReactElement => {
  const { t } = useStructureText();
  const hand = useHand();
  const asked = QUESTIONS.own(hand.scope);
  const write = writeMark(hand.level, marked);
  const read = usePreview(['proposal'], { plan: hand.clicks.tried(asked, ownOf(hand.scope), write) });
  if (read.status === 'loading') {
    return <p role="status" className="import-slow">{t('structure.counting')}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const own = ownNodeIn(read.data.proposal, hand.level);
  if (own instanceof Error) {
    return <ErrorNotice error={own} />;
  }
  return (
    <div className="guided-ask" aria-busy={read.busy}>
      {own === undefined ? <p role="status">{t('structure.change.notOffered')}</p> : <Makes items={own.items} />}
      <p className="import-row-actions">
        <button
          type="button"
          className="import-primary"
          disabled={!hand.editable || read.busy || own === undefined}
          onClick={() => {
            hand.clicks.answer(asked, 'own', ownOf(hand.scope), write);
            hand.onDone();
          }}
        >
          {t('structure.hand.use')}
        </button>
        <button type="button" onClick={hand.onDone}>
          {t('structure.hand.cancel')}
        </button>
      </p>
    </div>
  );
};

/** Marking an example of one folder: its files, the parts of the name, and the trial. */
export const HandPanel = (hand: Hand): ReactElement => {
  const { t } = useStructureText();
  return (
    <HandContext.Provider value={hand}>
      <section className="structure-hand" aria-label={t('structure.hand.title')}>
        <h5>{t('structure.hand.title')}</h5>
        <p>{t('structure.hand.lede')}</p>
        <ShowUsOneExample markFor={{ folder: hand.scope.folder, Trial: SeriesTrial }} />
      </section>
    </HandContext.Provider>
  );
};

/** A folder written by hand: whether it is imported, what it makes as written, the lists it keeps, the way back, and another mark. */
export const OwnInspector = ({ node, name }: { readonly node: OwnNode; readonly name: string }): ReactElement => {
  const { clicks, editable, words } = useScreen();
  const { t, number } = words;
  const [marking, setMarking] = useState(false);
  const scope = node.scope.scope;
  return (
    <div className="guided-inspect">
      <h4>{name}</h4>
      <dl className="guided-kv">
        <ImportQuestion scope={node.scope} items={node.items.items} />
      </dl>
      <p>{t('structure.own.note')}</p>
      <Makes items={node.items} />
      {(node.kept ?? []).map((kept) => (
        <p key={kept.list}>{keptWords(kept, t, number)}</p>
      ))}
      <p className="import-row-actions">
        <button
          type="button"
          disabled={!editable}
          onClick={() => {
            clicks.answer(QUESTIONS.own(scope), 'back', null);
          }}
        >
          {t('structure.own.back')}
        </button>
        {marking ? null : (
          <button
            type="button"
            disabled={!editable}
            onClick={() => {
              setMarking(true);
            }}
          >
            {t('structure.hand.mark')}
          </button>
        )}
      </p>
      {marking ? (
        <HandPanel
          clicks={clicks}
          editable={editable}
          level={node.level}
          scope={scope}
          onDone={() => {
            setMarking(false);
          }}
        />
      ) : null}
    </div>
  );
};
