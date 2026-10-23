/**
 * What "Your list" did, on a folder's node in the question zoom: where each
 * column of its lists goes, pair by hand, fill in a list yourself, and every
 * file of the folder. They edit the folder they are opened from.
 *
 * A column's field and options are carried members of a suggested folder:
 * the proposer copies them from the plan into its next proposal
 * (`preview/proposal.rs`, `kept_rules`), so a column choice here is written
 * into the plan and survives every click. A folder the stored plan does not
 * hold yet is edited in the shown structure (`shown-draft.tsx`), and its
 * lists are read from that structure too. The round trip asks the server
 * for the folder by its id, so it waits until the plan holds the folder.
 *
 * Pair by hand and Every file are sub-screens: their address names the node,
 * so Back returns to it. Where its item type comes from (`item-types.tsx`)
 * is drawn here too, so that chunk needs no Structure part. A chunk of its
 * own, loaded with the node.
 */
import { Suspense, lazy, useState } from 'react';
import type { ReactElement } from 'react';
import type { LevelId } from '../../../api/import-plan';
import { ErrorNotice } from '../../../app/errors';
import { DraftContext, useDraft } from '../draft-context';
import { listHref } from '../lists/address';
import { usePreview } from '../lists/use-preview';
import { nodeHref } from '../picked-node';
import { seriesOf, shownPlanFor } from '../series';
import { rollUpsOf } from './list-plan';
import { ListColumns } from './list-columns';
import { InShownTree, useShownDraft } from './shown-draft';
import { useScreen } from './structure-parts';

const RoundTrip = lazy(() => import('../lists/round-trip'));
const ItemTypes = lazy(() => import('./item-types'));

type Opened = 'columns' | 'trip' | null;

/** The table of each list of the folder `level`, read from the plan as the tree shows it. */
const Columns = ({ level }: { readonly level: LevelId }): ReactElement => {
  const { proposal, words } = useScreen();
  const { draft } = useShownDraft(level);
  const read = usePreview(['sheets', 'columnMatches', 'listCandidates'], shownPlanFor(draft, level));
  if (read.status === 'loading') {
    return <p className="import-slow">{words.t('shell.loading')}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const { sheets, columnMatches, listCandidates } = read.data;
  return (
    <DraftContext.Provider value={draft}>
      {seriesOf(draft.plan, level).lists.map((sheet) => {
        const view = sheets.find((one) => one.id === sheet.id);
        const candidate = listCandidates.find((one) => one.inPlan === sheet.id);
        const counts = { grain: candidate?.grain ?? null, matches: columnMatches.find((one) => one.sheet === sheet.id), cells: candidate?.cells ?? [], rollUps: rollUpsOf(proposal, sheet) };
        return view === undefined ? null : (
          <section key={sheet.id} aria-label={sheet.file}>
            {view.error === null ? null : <p role="alert">{view.error}</p>}
            <ListColumns sheet={sheet} view={view} counts={counts} />
          </section>
        );
      })}
    </DraftContext.Provider>
  );
};

export const ListHomes = ({ nodeKey, level, folder }: { readonly nodeKey: string; readonly level: LevelId; readonly folder: string }): ReactElement => {
  const { clicks, editable, words } = useScreen();
  const { t } = words;
  const { id } = useDraft();
  const { draft, held } = useShownDraft(level);
  const [opened, setOpened] = useState<Opened>(null);
  const hasList = seriesOf(draft.plan, level).lists.length > 0;
  const toggle = (which: Exclude<Opened, null>) => (): void => {
    setOpened((now) => (now === which ? null : which));
  };
  return (
    <>
      <p className="import-row-actions">
        {hasList ? (
          <>
            <button type="button" aria-expanded={opened === 'columns'} onClick={toggle('columns')}>
              {t('structure.columns.open')}
            </button>
            <a href={nodeHref(id, { mode: 'simple', screen: 'pair' }, nodeKey)}>{t('simple.pair.title')}</a>
          </>
        ) : null}
        <button type="button" aria-expanded={opened === 'trip'} onClick={toggle('trip')}>
          {t('lists.trip.title')}
        </button>
        <a href={listHref(id, { mode: 'simple', screen: 'all-files' }, { node: nodeKey, folder })}>{t('simple.allFiles.title')}</a>
      </p>
      {opened === 'columns' ? <Columns level={level} /> : null}
      {opened === 'trip' && held ? (
        <Suspense fallback={<p className="import-slow">{t('shell.loading')}</p>}>
          <RoundTrip level={level} />
        </Suspense>
      ) : null}
      {opened === 'trip' && !held ? (
        <p className="import-row-actions">
          <span>{t('structure.trip.first')}</span>
          <button type="button" disabled={!editable} onClick={clicks.use}>
            {t('structure.status.use')}
          </button>
        </p>
      ) : null}
    </>
  );
};

/** Where the folder `level`'s item type comes from, in the plan as the tree shows it. */
export const ItemTypesHome = ({ level }: { readonly level: LevelId }): ReactElement => (
  <InShownTree level={level}>
    <Suspense fallback={null}>
      <ItemTypes level={level} />
    </Suspense>
  </InShownTree>
);
