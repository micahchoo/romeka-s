/**
 * "Do you have a list that describes them?": a list found in the folder,
 * used only when the person says so; how its rows find their items, by the
 * column with the most cells that exactly equal an item's code or a file's
 * name, counted on screen; where each column goes; and the list to fill in
 * yourself.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import type { SheetColumnMatches } from '../../../api/import-sections';
import type { SheetView } from '../../../api/import-sheet-views';
import type { SheetPlan } from '../../../api/import-plan';
import { ErrorNotice } from '../../../app/errors';
import { toError } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { RoundTrip, removeSheet } from '../lists/round-trip';
import { usePreview } from '../lists/use-preview';
import { placeHref } from '../screens';
import { declines, readState, withState } from './example-state';
import { ListColumns } from './list-columns';
import { cellsOf, withJoin } from './list-plan';
import { folderSheet, listEdit, listFiles } from './list-read';
import { useSimpleText } from './messages';
import './simple.css';

const Offer = ({ source, file, declined }: { readonly source: string; readonly file: string; readonly declined: boolean }): ReactElement => {
  const { t } = useSimpleText();
  const draft = useDraft();
  const [state, setState] = useState<{ readonly busy: boolean; readonly error: Error | null }>({ busy: false, error: null });
  const use = (): void => {
    setState({ busy: true, error: null });
    listEdit(draft.id, draft.plan, { source, file })
      .then((edit) => {
        draft.change((plan) => edit(withState(plan, { notLists: readState(plan).notLists.filter((one) => !declines(one, source, file)) })));
        setState({ busy: false, error: null });
      })
      .catch((error: unknown) => {
        setState({ busy: false, error: toError(error) });
      });
  };
  return (
    <div className="guided-ask" data-decision="list">
      <p className="guided-big">{t('guided.list.found', { file })}</p>
      <p className="import-row-actions">
        <button type="button" className="import-primary" aria-pressed="false" disabled={!draft.editable || state.busy} onClick={use}>
          {state.busy ? t('guided.list.reading') : t('guided.list.yes')}
        </button>
        <button
          type="button"
          aria-pressed={declined}
          disabled={!draft.editable || state.busy || declined}
          onClick={() => {
            draft.change((plan) => withState(plan, { notLists: [...readState(plan).notLists, { source, file }] }));
          }}
        >
          {t('guided.list.no')}
        </button>
      </p>
      {state.error === null ? null : <ErrorNotice error={state.error} />}
    </div>
  );
};

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
  const first = view.lines[0];
  const delimiter = view.read?.delimiter ?? ',';
  if (first === undefined || (sheet.read.headerLine === 1 && (view.guess?.headerLine ?? 1) === 1)) {
    return null;
  }
  const cells = cellsOf(first, delimiter === 'auto' ? ',' : delimiter).filter((cell) => cell.trim() !== '');
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

const Found = ({ unpairedRows, unpairedFiles, rows }: { readonly unpairedRows: number; readonly unpairedFiles: number; readonly rows: number }): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  return (
    <p>
      <b>{t('guided.list.paired', { count: number(rows - unpairedRows), rows: number(rows) })}</b>{' '}
      {unpairedRows === 0 ? null : <>{unpairedRows === 1 ? t('guided.list.noFileOne') : t('guided.list.noFile', { count: number(unpairedRows) })} </>}
      {unpairedFiles === 0 ? null : <>{unpairedFiles === 1 ? t('guided.list.noRowOne') : t('guided.list.noRow', { count: number(unpairedFiles) })} </>}
      {unpairedRows + unpairedFiles === 0 ? null : <a href={placeHref(draft.id, { mode: 'simple', screen: 'pair' })}>{t('guided.list.pair')}</a>}
    </p>
  );
};

const InUse = ({ sheet }: { readonly sheet: SheetPlan }): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const read = usePreview(['sheets', 'columnMatches', 'unpaired'], { unpaired: { rows: { perPage: 1 }, files: { perPage: 1 } } });
  if (read.status === 'loading') {
    return <p role="status" className="import-slow">{t('shell.loading')}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const view = read.data.sheets.find((one) => one.id === sheet.id);
  const matches = read.data.columnMatches.find((one) => one.sheet === sheet.id);
  const rows = view?.read?.rows ?? matches?.rows ?? 0;
  return (
    <section aria-busy={read.busy}>
      <div className="guided-ask" data-decision="list">
        <p className="guided-big">{t('guided.list.foundRows', { file: sheet.file, count: number(rows) })}</p>
        <p className="import-row-actions">
          <button type="button" className="import-primary" aria-pressed="true" disabled={!draft.editable}>
            {t('guided.list.yes')}
          </button>
          <button
            type="button"
            aria-pressed="false"
            disabled={!draft.editable}
            onClick={() => {
              draft.change((plan) => withState(removeSheet(plan, sheet.id), { notLists: [...readState(plan).notLists, { source: sheet.source, file: sheet.file }] }));
            }}
          >
            {t('guided.list.no')}
          </button>
        </p>
      </div>
      {view?.error === null || view === undefined ? null : <p role="alert">{view.error}</p>}
      <ul className="muted">
        {view === undefined || encodingWords(view, t) === null ? null : <li>{encodingWords(view, t)}</li>}
        {view === undefined ? null : <HeaderLine sheet={sheet} view={view} />}
      </ul>
      <Pairing sheet={sheet} matches={matches} rows={rows} />
      <Found unpairedRows={read.data.unpaired.rows.total} unpairedFiles={read.data.unpaired.files.total} rows={rows} />
      {view === undefined ? null : <ListColumns sheet={sheet} view={view} />}
    </section>
  );
};

export const YourList = (): ReactElement => {
  const { t } = useSimpleText();
  const draft = useDraft();
  const read = usePreview(['histogram'], {});
  const sheet = folderSheet(draft.plan);
  if (read.status === 'loading') {
    return <p role="status" className="import-slow">{t('shell.loading')}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const offers = listFiles(draft.plan, read.data.sources);
  const { notLists } = readState(draft.plan);
  return (
    <>
      {sheet === undefined ? null : <InUse sheet={sheet} />}
      {sheet === undefined && offers.length === 0 ? (
        <div className="import-note">
          <p>{t('guided.list.none')}</p>
          <p className="muted">{t('guided.list.what')}</p>
        </div>
      ) : null}
      {sheet === undefined ? offers.slice(0, 1).map((offer) => <Offer key={`${offer.source}/${offer.file}`} source={offer.source} file={offer.file} declined={notLists.some((entry) => declines(entry, offer.source, offer.file))} />) : null}
      <RoundTrip />
    </>
  );
};

export default YourList;
