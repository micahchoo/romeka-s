/**
 * "Do you have a list that describes them?": the lists found in the folder,
 * each used only when the person says so; for each list in use, how its rows
 * find their items, by the column with the most cells that exactly equal an
 * item's code or a file's name, counted on screen, what one row describes,
 * and where each column goes; and the list to fill in yourself.
 */
import type { ReactElement } from 'react';
import type { Grain, SheetColumnMatches } from '../../../api/import-sections';
import type { SheetView } from '../../../api/import-sheet-views';
import type { RecordKind, SheetPlan } from '../../../api/import-plan';
import { ErrorNotice } from '../../../app/errors';
import { useDraft } from '../draft-context';
import { ListPicker } from '../lists/list-picker';
import { RoundTrip } from '../lists/round-trip';
import { usePreview } from '../lists/use-preview';
import { placeHref } from '../screens';
import { ListColumns } from './list-columns';
import { delimiterOf, recordsOf, withFileRows, withJoin, withRowKind } from './list-plan';
import type { ListCounts } from './list-plan';
import { folderSheets } from './list-read';
import { useSimpleText } from './messages';
import type { SimpleMessageKey } from './messages';
import './simple.css';

const encodingWords = (view: SheetView, t: ReturnType<typeof useSimpleText>['t']): string | null => {
  const encoding = view.read?.encoding.encoding;
  if (encoding === undefined) {
    return null;
  }
  return encoding === 'utf-8' ? t('guided.list.utf8') : t('guided.list.notUtf8', { kind: encoding });
};

const HeaderLine = ({ sheet, view }: { readonly sheet: SheetPlan; readonly view: SheetView }): ReactElement | null => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const first = recordsOf(view.lines, delimiterOf(view)).find((record) => record.line === 1);
  if (first === undefined || (sheet.read.headerLine === 1 && (view.guess?.headerLine ?? 1) === 1)) {
    return null;
  }
  const cells = first.cells.filter((cell) => cell.trim() !== '');
  return (
    <li data-decision="headerline">
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
              draft.change((plan) => ({ ...plan, sheets: plan.sheets.map((one) => (one.id === sheet.id ? { ...one, read: { ...one.read, headerLine: line } } : one)) }));
            }
          }}
        />
      </label>
    </li>
  );
};

const Pairing = ({ sheet, matches, rows }: { readonly sheet: SheetPlan; readonly matches: SheetColumnMatches | undefined; readonly rows: number }): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const columns = matches?.columns ?? [];
  const counted = columns.filter((column) => column.groupKeys > 0 || column.fileNames > 0);
  return (
    <>
      <h4>{t('guided.list.howTitle')}</h4>
      <label data-decision="join">
        {t('guided.list.pairBy')}{' '}
        <select
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
      </label>
      <ul>
        {counted.map((column) => (
          <li key={column.index}>
            <b>{column.header}</b>:{' '}
            {column.fileNames >= column.groupKeys
              ? t('guided.list.names', { count: number(column.fileNames), filled: number(rows - column.empty) })
              : t('guided.list.codes', { count: number(column.groupKeys), filled: number(rows - column.empty) })}
          </li>
        ))}
      </ul>
      <p className="muted">{t('guided.list.codeGloss')}</p>
    </>
  );
};

/**
 * How the rows found their items: the rows that name no file of the import,
 * the rows whose file this Omeka refuses (the `listed-not-media` flags, which
 * Check states), and the items with no row.
 */
const Found = ({ unpairedRows, refused, items, rows }: { readonly unpairedRows: number; readonly refused: number; readonly items: number; readonly rows: number }): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  return (
    <p>
      <b>{t('guided.list.paired', { count: number(rows - unpairedRows - refused), rows: number(rows) })}</b>{' '}
      {unpairedRows === 0 ? null : <>{unpairedRows === 1 ? t('guided.list.noImportFileOne') : t('guided.list.noImportFile', { count: number(unpairedRows) })} </>}
      {refused === 0 ? null : (
        <>
          {refused === 1 ? t('guided.list.refusedOne') : t('guided.list.refused', { count: number(refused) })}{' '}
          <a href={placeHref(draft.id, { mode: 'simple', screen: 'check' })}>{t('guided.list.refusedCheck')}</a>{' '}
        </>
      )}
      {items === 0 ? null : <>{items === 1 ? t('guided.list.noRowOne') : t('guided.list.noRow', { count: number(items) })} </>}
      {unpairedRows + items === 0 ? null : <a href={placeHref(draft.id, { mode: 'simple', screen: 'pair' })}>{t('guided.list.pair')}</a>}
    </p>
  );
};

const KINDS: readonly (readonly [RecordKind, SimpleMessageKey])[] = [
  ['item', 'guided.list.grainItem'],
  ['media', 'guided.list.grainFile'],
  ['itemSet', 'guided.list.grainSet'],
];

/** "Each row describes": the answer the plan holds, and the one the count suggests. */
const RowsDescribe = ({ sheet, grain }: { readonly sheet: SheetPlan; readonly grain: Grain | null }): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const others = grain?.rowsNamingOtherFilesOfAGroup ?? 0;
  const suggested: RecordKind = others > 0 ? 'media' : 'item';
  const evidence = grain === null ? null : others === 0 ? t('guided.list.grainNone') : others === 1 ? t('guided.list.grainEvidenceOne') : t('guided.list.grainEvidence', { count: number(others) });
  return (
    <fieldset className="guided-opts" data-decision="rowKind">
      <legend>{t('guided.list.grainAsk')}</legend>
      {evidence === null ? null : <p>{evidence}</p>}
      {KINDS.map(([kind, label]) => (
        <label key={kind}>
          <input
            type="radio"
            name={`rows-${sheet.id}`}
            value={kind}
            checked={sheet.rowKind === kind}
            disabled={!draft.editable}
            onChange={() => {
              draft.change((plan) => (kind === 'media' ? withFileRows(plan, sheet.id, grain) : withRowKind(plan, sheet.id, kind)));
            }}
          />
          <span>
            {t(label)}
            {kind === suggested ? <span className="guided-pill">{t('guided.check.suggested')}</span> : null}
          </span>
        </label>
      ))}
    </fieldset>
  );
};

const InUse = ({
  sheet,
  view,
  counts,
}: {
  readonly sheet: SheetPlan;
  readonly view: SheetView | undefined;
  readonly counts: ListCounts;
}): ReactElement => {
  const { matches, grain } = counts;
  const { t } = useSimpleText();
  const rows = view?.read?.rows ?? matches?.rows ?? 0;
  return (
    <section aria-label={sheet.file}>
      <h4>{sheet.file}</h4>
      {view?.error === null || view === undefined ? null : <p role="alert">{view.error}</p>}
      <ul className="muted">
        {view === undefined || encodingWords(view, t) === null ? null : <li>{encodingWords(view, t)}</li>}
        {view === undefined ? null : <HeaderLine sheet={sheet} view={view} />}
      </ul>
      <Pairing sheet={sheet} matches={matches} rows={rows} />
      <RowsDescribe sheet={sheet} grain={grain} />
      {view === undefined ? null : <ListColumns sheet={sheet} view={view} counts={counts} />}
    </section>
  );
};

/** The lists in use: one block each, under the unpaired counts, which the server totals over every list. */
const InUseLists = ({ sheets }: { readonly sheets: readonly SheetPlan[] }): ReactElement => {
  const { t } = useSimpleText();
  const read = usePreview(['sheets', 'columnMatches', 'unpaired', 'listCandidates', 'flags'], { unpaired: { rows: { perPage: 1 }, files: { perPage: 1 } } });
  if (read.status === 'loading') {
    return <p role="status" className="import-slow">{t('shell.loading')}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const blocks = sheets.map((sheet) => {
    const candidate = read.data.listCandidates.find((one) => one.inPlan === sheet.id);
    const counts: ListCounts = {
      grain: candidate?.grain ?? null,
      matches: read.data.columnMatches.find((one) => one.sheet === sheet.id),
      cells: candidate?.cells ?? [],
    };
    return { sheet, view: read.data.sheets.find((one) => one.id === sheet.id), counts };
  });
  const rows = blocks.reduce((sum, { view, counts }) => sum + (view?.read?.rows ?? counts.matches?.rows ?? 0), 0);
  const refused = read.data.flags.filter((flag) => flag.kind === 'listed-not-media').reduce((sum, flag) => sum + flag.count, 0);
  return (
    <div aria-busy={read.busy}>
      <Found unpairedRows={read.data.unpaired.rows.total} refused={refused} items={read.data.unpaired.items} rows={rows} />
      {blocks.map(({ sheet, view, counts }) => (
        <InUse key={sheet.id} sheet={sheet} view={view} counts={counts} />
      ))}
    </div>
  );
};

export const YourList = (): ReactElement => {
  const { t } = useSimpleText();
  const draft = useDraft();
  const sheets = folderSheets(draft.plan);
  return (
    <>
      <ListPicker
        none={
          <div className="import-note">
            <p>{t('guided.list.none')}</p>
            <p className="muted">{t('guided.list.what')}</p>
          </div>
        }
      />
      {sheets.length === 0 ? null : <InUseLists sheets={sheets} />}
      <RoundTrip />
    </>
  );
};

export default YourList;
