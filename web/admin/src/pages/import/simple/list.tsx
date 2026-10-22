/**
 * "Your list": a one-line count of what the lists in use pair, then every
 * list the folder holds beside the inspector of the one selected. For a list
 * the inspector says whether it is used, how many rows find a file or an item
 * (and how many more would with names compared loosely), which column pairs
 * them, which list in use pairs the same items, what one row describes, and
 * where each column goes. The list to fill in yourself waits, collapsed,
 * below.
 */
import type { ReactElement } from 'react';
import type { ListCandidate, SheetColumnMatches } from '../../../api/import-sections';
import type { SheetView } from '../../../api/import-sheet-views';
import type { LevelId, RecordKind, SheetPlan } from '../../../api/import-plan';
import { ErrorNotice } from '../../../app/errors';
import { useDraft } from '../draft-context';
import { nameOf, stateOf, stateWords, useListChoice } from '../lists/list-picker';
import type { ListState } from '../lists/list-picker';
import { RoundTrip } from '../lists/round-trip';
import { usePreview } from '../lists/use-preview';
import { placeHref } from '../screens';
import { ListColumns } from './list-columns';
import { linesOf, normalizedOf, withFileRows, withJoin, withNormalize, withRowKind } from './list-plan';
import type { ListCounts } from './list-plan';
import { folderSheets } from './list-read';
import { MasterDetail, Segmented } from './master-detail';
import { useSimpleText } from './messages';
import type { SimpleMessageKey } from './messages';
import { usePickedSeries } from '../picked-series';
import { seriesOf, withList } from '../series';
import './simple.css';

const keyOf = (candidate: ListCandidate): string => `${candidate.source}/${candidate.file}`;

/** Whether the list pairs with every name fold on: only a list in use can. */
const looseOf = (state: ListState): boolean => state.kind === 'inUse' && normalizedOf(state.sheet);

const Meter = ({ count, of }: { readonly count: number; readonly of: number }): ReactElement => {
  const { number } = useSimpleText();
  const share = of === 0 ? 0 : (count / of) * 100;
  return (
    <span className="guided-meter">
      <span className="guided-bar" aria-hidden="true">
        <span className={share < 60 ? 'is-low' : undefined} style={{ width: `${String(share)}%` }} />
      </span>
      <span>
        {number(count)} / {number(of)}
      </span>
    </span>
  );
};

const Matched = ({ candidate, state }: { readonly candidate: ListCandidate; readonly state: ListState }): ReactElement => {
  const { best } = candidate;
  const count = best === null ? 0 : looseOf(state) ? best.pairedNormalized : best.paired;
  return <Meter count={count} of={candidate.rows} />;
};

/** "+K ignoring case, zeros and _ - .": a toggle for a list in use that pairs by a column, a fact for any other. */
const LooseChip = ({ candidate, state }: { readonly candidate: ListCandidate; readonly state: ListState }): ReactElement | null => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const { best } = candidate;
  if (best === null || best.pairedNormalized <= best.paired) {
    return null;
  }
  const words = t('lists.picker.loose', { count: number(best.pairedNormalized - best.paired) });
  if (state.kind !== 'inUse' || state.sheet.join === null) {
    return <span className="guided-chip">{words}</span>;
  }
  const sheet = state.sheet;
  const on = normalizedOf(sheet);
  return (
    <button
      type="button"
      className="guided-chip"
      aria-pressed={on}
      disabled={!draft.editable}
      onClick={() => {
        draft.change((plan) => withNormalize(plan, sheet.id, !on));
      }}
    >
      {words}
    </button>
  );
};

const encodingWords = (view: SheetView, t: ReturnType<typeof useSimpleText>['t']): string | null => {
  const encoding = view.read?.encoding.encoding;
  if (encoding === undefined || view.read?.workbook != null) {
    return null;
  }
  return encoding === 'utf-8' ? t('guided.list.utf8') : t('guided.list.readAs', { kind: encoding });
};

const HeaderLine = ({ sheet, view }: { readonly sheet: SheetPlan; readonly view: SheetView }): ReactElement | null => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const first = linesOf(view).records.find((record) => record.line === 1);
  if (first === undefined || (sheet.read.headerLine === 1 && (view.guess?.headerLine ?? 1) === 1)) {
    return null;
  }
  const cells = first.cells.filter((cell) => cell.trim() !== '');
  return (
    <p data-decision="headerline">
      {cells.length === 1 ? t('guided.list.line1One', { text: cells[0] ?? '' }) : t('guided.list.line1', { count: number(cells.length) })}{' '}
      <label>
        {t('guided.list.headerLine')}{' '}
        <input
          type="number"
          min={1}
          max={Math.max(1, view.lines.length)}
          value={sheet.read.headerLine}
          disabled={!draft.editable}
          style={{ width: '5rem' }}
          onChange={(event) => {
            const line = Number(event.target.value);
            if (Number.isInteger(line) && line >= 1) {
              draft.change((plan) =>
                withList(plan, sheet.id, (one) => ({ ...one, read: { ...one.read, headerLine: line } })),
              );
            }
          }}
        />
      </label>
    </p>
  );
};

const Pairing = ({ sheet, matches }: { readonly sheet: SheetPlan; readonly matches: SheetColumnMatches | undefined }): ReactElement => {
  const { t } = useSimpleText();
  const draft = useDraft();
  const columns = matches?.columns ?? [];
  return (
    <select
      aria-label={t('guided.list.pairBy')}
      data-decision="join"
      value={sheet.join === null ? '' : String(sheet.join.column.index)}
      disabled={!draft.editable}
      onChange={(event) => {
        const column = columns.find((one) => String(one.index) === event.target.value);
        if (column !== undefined) {
          draft.change((plan) => withJoin(plan, sheet.id, column));
        }
      }}
    >
      {sheet.join === null ? <option value="">{t('guided.list.pairNone')}</option> : null}
      {columns.map((column) => (
        <option key={column.index} value={String(column.index)}>
          {column.header}
        </option>
      ))}
    </select>
  );
};

const KINDS: readonly (readonly [RecordKind, SimpleMessageKey])[] = [
  ['item', 'guided.list.kindItem'],
  ['media', 'guided.list.kindFile'],
  ['itemSet', 'guided.list.kindSet'],
];

/** "A row is": the answer the plan holds, the one the count suggests marked, and the count when they differ. */
const RowIs = ({ sheet, candidate }: { readonly sheet: SheetPlan; readonly candidate: ListCandidate }): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const others = candidate.grain?.rowsNamingOtherFilesOfAGroup ?? 0;
  const suggested: RecordKind = others > 0 ? 'media' : 'item';
  const value = typeof sheet.rowKind === 'string' ? sheet.rowKind : null;
  let evidence: string | null = null;
  if (candidate.grain !== null && value !== suggested) {
    evidence = others === 0 ? t('guided.list.oneEach') : others === 1 ? t('guided.list.otherFilesOne') : t('guided.list.otherFiles', { count: number(others) });
  }
  return (
    <>
      <Segmented
        label={t('guided.list.grainAsk')}
        segments={KINDS.map(([kind, label]) => ({ value: kind, label: t(label), suggested: kind === suggested }))}
        value={value}
        disabled={!draft.editable}
        suggestedWord={t('guided.check.suggested')}
        onChange={(kind) => {
          draft.change((plan) => (kind === 'media' ? withFileRows(plan, sheet.id) : withRowKind(plan, sheet.id, kind)));
        }}
      />
      {evidence === null ? null : <span className="muted">{evidence}</span>}
    </>
  );
};

interface Read {
  readonly sheets: readonly SheetView[];
  readonly columnMatches: readonly SheetColumnMatches[];
  readonly listCandidates: readonly ListCandidate[];
}

/** One list: whether it is used, what it pairs and, when in use, how. */
const Inspector = ({ level, candidate, read }: { readonly level: LevelId; readonly candidate: ListCandidate; readonly read: Read }): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const choice = useListChoice(candidate, level);
  const { state } = choice;
  const sheet = state.kind === 'inUse' ? state.sheet : null;
  const view = sheet === null ? undefined : read.sheets.find((one) => one.id === sheet.id);
  const counts: ListCounts | null =
    sheet === null ? null : { grain: candidate.grain, matches: read.columnMatches.find((one) => one.sheet === sheet.id), cells: candidate.cells };
  const overlaps = candidate.overlaps.filter((overlap) => seriesOf(draft.plan, level).lists.some((one) => one.source === overlap.source && one.file === overlap.file));
  const { name, folder } = nameOf(candidate.file);
  const encoding = view === undefined ? null : encodingWords(view, t);
  return (
    <section aria-label={candidate.file} className="guided-inspect">
      <h4>
        <span className="guided-file">{name}</span> {folder === '' ? null : <span className="muted">{folder}/</span>}
      </h4>
      {candidate.read === 'ok' ? null : <p role="alert">{t('lists.picker.unread', { message: candidate.read.error })}</p>}
      {view?.error == null ? null : <p role="alert">{view.error}</p>}
      {candidate.read === 'ok' ? (
        <dl className="guided-kv">
          <dt>{t('guided.list.use')}</dt>
          <dd data-decision="list">
            <label className="guided-tgl">
              <input
                type="checkbox"
                checked={state.kind === 'inUse'}
                disabled={choice.busy || choice.asking}
                onChange={(event) => {
                  if (event.target.checked) {
                    choice.use();
                  } else {
                    choice.stop();
                  }
                }}
              />
              {t('guided.list.useThis')}
            </label>
            {choice.reading ? <span role="status">{t('lists.picker.reading')}</span> : null}
            {state.kind === 'offered' && !choice.reading ? (
              <button type="button" className="link-button" disabled={choice.busy} onClick={choice.decline}>
                {t('lists.picker.decline')}
              </button>
            ) : null}
          </dd>
          <dt>{t('guided.list.rowsPaired')}</dt>
          <dd>
            <Matched candidate={candidate} state={state} />
            <LooseChip candidate={candidate} state={state} />
            {candidate.best === null ? <span className="muted">{t('lists.picker.none')}</span> : null}
          </dd>
          {overlaps.length === 0 ? null : (
            <>
              <dt>{t('guided.list.overlap')}</dt>
              <dd>
                {overlaps.map((overlap) => (
                  <span key={`${overlap.source}/${overlap.file}`} className="guided-chip is-warn">
                    {t('lists.picker.sameItems', { file: overlap.file, count: number(overlap.sharedTargets) })}
                  </span>
                ))}
              </dd>
            </>
          )}
          {sheet === null || counts === null ? null : (
            <>
              <dt>{t('guided.list.pairing')}</dt>
              <dd>
                <Pairing sheet={sheet} matches={counts.matches} />
              </dd>
              <dt>{t('guided.list.rowIs')}</dt>
              <dd data-decision="rowKind">
                <RowIs sheet={sheet} candidate={candidate} />
              </dd>
            </>
          )}
        </dl>
      ) : null}
      {choice.notice}
      {encoding === null ? null : <p className="muted">{encoding}</p>}
      {sheet === null || view === undefined ? null : <HeaderLine sheet={sheet} view={view} />}
      {sheet === null || view === undefined || counts === null ? null : <ListColumns sheet={sheet} view={view} counts={counts} />}
    </section>
  );
};

/**
 * The counts over every list in use, which the server totals: the rows that
 * found their item, the rows that name no file of the import, the rows whose
 * file this Omeka refuses (the `listed-not-media` flags, which Check states),
 * and the items with no row.
 */
const Summary = ({ lists, rows, unpairedRows, refused, items }: { readonly lists: number; readonly rows: number; readonly unpairedRows: number; readonly refused: number; readonly items: number }): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  return (
    <p className="guided-facts" role="status">
      <span>
        {t('guided.list.inUseCount')} <b>{number(lists)}</b>
      </span>
      {lists === 0 ? null : (
        <>
          <span>
            {t('guided.list.rowsPaired')}{' '}
            <b>
              {number(Math.max(0, rows - unpairedRows - refused))} / {number(rows)}
            </b>
          </span>
          <span>
            {t('guided.list.noFileRows')} <b>{number(unpairedRows)}</b>
          </span>
          {refused === 0 ? null : (
            <span>
              {t('guided.list.refusedRows')} <b>{number(refused)}</b> <a href={placeHref(draft.id, { mode: 'simple', screen: 'check' })}>{t('simple.check.step')}</a>
            </span>
          )}
          <span>
            {t('guided.list.noRowItems')} <b>{number(items)}</b>
            {unpairedRows + items === 0 ? null : (
              <>
                {' '}
                <a href={placeHref(draft.id, { mode: 'simple', screen: 'pair' })}>{t('guided.list.pairHand')}</a>
              </>
            )}
          </span>
        </>
      )}
    </p>
  );
};

const Help = (): ReactElement => {
  const { t } = useSimpleText();
  return (
    <details className="guided-help">
      <summary>{t('guided.list.help')}</summary>
      <p>{t('guided.list.helpPaired')}</p>
      <p>{t('guided.list.codeGloss')}</p>
      <p>{t('guided.list.helpRowIs')}</p>
      <p>{t('guided.list.columnsLede')}</p>
      <p>{t('guided.list.levelLede')}</p>
      <p>{t('guided.list.moreFields')}</p>
    </details>
  );
};

const Lists = ({ level }: { readonly level: LevelId }): ReactElement => {
  const { t } = useSimpleText();
  const draft = useDraft();
  const read = usePreview(['listCandidates', 'sheets', 'columnMatches', 'unpaired', 'flags'], { unpaired: { rows: { perPage: 1 }, files: { perPage: 1 } } });
  if (read.status === 'loading') {
    return <p role="status" className="import-slow">{t('shell.loading')}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const candidates = read.data.listCandidates;
  if (candidates.length === 0) {
    return (
      <div className="import-note">
        <p>{t('guided.list.none')}</p>
        <p className="muted">{t('guided.list.whatShort')}</p>
      </div>
    );
  }
  const sheets = folderSheets(draft.plan, level);
  const rows = sheets.reduce((sum, sheet) => {
    const view = read.data.sheets.find((one) => one.id === sheet.id);
    return sum + (view?.read?.rows ?? read.data.columnMatches.find((one) => one.sheet === sheet.id)?.rows ?? 0);
  }, 0);
  const refused = read.data.flags.filter((flag) => flag.kind === 'listed-not-media').reduce((sum, flag) => sum + flag.count, 0);
  const states = candidates.map((candidate) => ({ candidate, state: stateOf(draft.plan, candidate, level) }));
  const inUse = states.find(({ state }) => state.kind === 'inUse');
  return (
    <div aria-busy={read.busy} className="guided-dials">
      <Summary lists={sheets.length} rows={rows} unpairedRows={read.data.unpaired.rows.total} refused={refused} items={read.data.unpaired.items} />
      <MasterDetail
        label={t('lists.picker.title')}
        initial={inUse === undefined ? null : keyOf(inUse.candidate)}
        rows={states.map(({ candidate, state }) => ({
          key: keyOf(candidate),
          name: <span className="guided-file">{candidate.file}</span>,
          facts: (
            <>
              {state.kind === 'offered' ? null : <span className={state.kind === 'inUse' ? 'guided-chip is-ok' : 'guided-chip'}>{stateWords(state, t)}</span>}
              {candidate.read === 'ok' ? <Matched candidate={candidate} state={state} /> : null}
            </>
          ),
        }))}
      >
        {(key) => {
          const candidate = candidates.find((one) => keyOf(one) === key);
          return candidate === undefined ? null : <Inspector key={key} level={level} candidate={candidate} read={read.data} />;
        }}
      </MasterDetail>
    </div>
  );
};

/** The plan's one series: this screen opens only on a plan of one (`screens.ts`, `landing`). */
export const YourList = (): ReactElement | null => {
  const { t } = useSimpleText();
  const series = usePickedSeries();
  if (series === null) {
    return null;
  }
  return (
    <>
      <Lists level={series.id} />
      <Help />
      <details className="guided-trip">
        <summary>{t('lists.trip.title')}</summary>
        <RoundTrip level={series.id} />
      </details>
    </>
  );
};

export default YourList;
