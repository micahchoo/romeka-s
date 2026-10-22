/**
 * Advanced, Lists: the list files of the folder to choose from, as Simple
 * shows them; then for each list of the import, how it is read, which column
 * pairs its rows with items (chosen by exact counts) and how equal two keys
 * must be, what each row describes, the rows and items left unpaired, and
 * its columns with field, split, type, language, private and, for rows that
 * describe files, where their values go. Then the list round trip.
 */
import { lazy, Suspense, useId } from 'react';
import type { ReactElement } from 'react';
import { DELIMITERS, ENCODINGS } from '../../../api/import-plan';
import type { ColumnRef, Delimiter, Encoding, Join, Quote, ReadOptions, SheetPlan } from '../../../api/import-plan';
import type { SheetColumnMatches } from '../../../api/import-sections';
import type { SheetView } from '../../../api/import-sheet-views';
import { ErrorNotice } from '../../../app/errors';
import { useDraft } from '../draft-context';
import { ListPicker } from '../lists/list-picker';
import { usePreview } from '../lists/use-preview';
import { placeHref } from '../screens';
import { ColumnsTable } from './columns-table';
import { useAdvancedText } from './messages';
import type { AdvancedMessageKey, AdvancedTranslator } from './messages';
import { withRowKind } from '../simple/list-plan';
import type { ListCounts } from '../simple/list-plan';
import { NO_NORMALIZE, setItemKey, setJoin, setJoinColumn, setRead, setRowKindColumn } from './plan-edits';
import { Setting } from './setting';
import { seriesOf } from '../series';
import './advanced.css';

const RoundTrip = lazy(() => import('../lists/round-trip'));

const DELIMITER_WORDS: Readonly<Record<Delimiter, AdvancedMessageKey>> = {
  auto: 'adv.lists.delimiter.auto',
  ',': 'adv.lists.delimiter.comma',
  ';': 'adv.lists.delimiter.semicolon',
  '\t': 'adv.lists.delimiter.tab',
  '|': 'adv.lists.delimiter.pipe',
  ':': 'adv.lists.delimiter.colon',
  ' ': 'adv.lists.delimiter.space',
};

const QUOTES: readonly (readonly [Quote, AdvancedMessageKey])[] = [
  ['"', 'adv.lists.quote.double'],
  ["'", 'adv.lists.quote.single'],
  ['none', 'adv.lists.quote.none'],
];

const NORMALIZE: readonly (readonly [Exclude<keyof Join['normalize'], 'lastPlaces'>, AdvancedMessageKey])[] = [
  ['caseFold', 'adv.lists.normalize.caseFold'],
  ['leadingZeros', 'adv.lists.normalize.leadingZeros'],
  ['separators', 'adv.lists.normalize.separators'],
];

const encodingOf = (value: string): ReadOptions['encoding'] => ENCODINGS.find((one) => one === value) ?? 'auto';
const delimiterOf = (value: string): Delimiter => DELIMITERS.find((one) => one === value) ?? 'auto';
const quoteOf = (value: string): Quote => QUOTES.find(([one]) => one === value)?.[0] ?? '"';

const ENCODING_NAMES: Readonly<Record<Encoding, string>> = {
  'utf-8': 'UTF-8',
  'utf-16le': 'UTF-16 LE',
  'utf-16be': 'UTF-16 BE',
  'windows-1252': 'Windows-1252',
  'iso-8859-1': 'ISO-8859-1',
};

/** "Id: 805 of 812 cells are an item's identifier, 0 a file name". */
const matchWords = (match: SheetColumnMatches['columns'][number], rows: number, t: AdvancedTranslator['t'], number: (n: number) => string): string =>
  t('adv.lists.join.column', { header: match.header, keys: number(match.groupKeys), files: number(match.fileNames), rows: number(rows) });

/** A count in words: none, one, or many. */
const countWords = (
  count: number,
  [none, one, many]: readonly [AdvancedMessageKey, AdvancedMessageKey, AdvancedMessageKey],
  t: AdvancedTranslator['t'],
  number: (n: number) => string,
): string => (count === 0 ? t(none) : count === 1 ? t(one) : t(many, { count: number(count) }));

const Reading = ({ sheet, view }: { readonly sheet: SheetPlan; readonly view: SheetView | undefined }): ReactElement => {
  const { t } = useAdvancedText();
  const draft = useDraft();
  const read = (options: Partial<ReadOptions>): void => {
    draft.change((plan) => setRead(plan, sheet.id, options));
  };
  const guessed = view?.guess?.encoding?.encoding;
  const lineId = useId();
  return (
    <>
      <Setting label={t('adv.lists.encoding.label')} plain={guessed === undefined ? t('adv.lists.encoding.plain') : t('adv.lists.encoding.guess', { encoding: ENCODING_NAMES[guessed] })}>
        {(described) => (
          <select aria-label={t('adv.lists.encoding.label')} aria-describedby={described} value={sheet.read.encoding} disabled={!draft.editable} onChange={(event) => { read({ encoding: encodingOf(event.target.value) }); }}>
            <option value="auto">{t('adv.lists.encoding.auto')}</option>
            {ENCODINGS.map((encoding) => (
              <option key={encoding} value={encoding}>
                {ENCODING_NAMES[encoding]}
              </option>
            ))}
          </select>
        )}
      </Setting>
      <Setting label={t('adv.lists.delimiter.label')} plain={t('adv.lists.delimiter.plain')}>
        {(described) => (
          <select aria-label={t('adv.lists.delimiter.label')} aria-describedby={described} value={sheet.read.delimiter} disabled={!draft.editable} onChange={(event) => { read({ delimiter: delimiterOf(event.target.value) }); }}>
            {DELIMITERS.map((delimiter) => (
              <option key={delimiter} value={delimiter}>
                {t(DELIMITER_WORDS[delimiter])}
              </option>
            ))}
          </select>
        )}
      </Setting>
      <Setting label={t('adv.lists.quote.label')} plain={t('adv.lists.quote.plain')}>
        {(described) => (
          <select aria-label={t('adv.lists.quote.label')} aria-describedby={described} value={sheet.read.quote} disabled={!draft.editable} onChange={(event) => { read({ quote: quoteOf(event.target.value) }); }}>
            {QUOTES.map(([quote, key]) => (
              <option key={quote} value={quote}>
                {t(key)}
              </option>
            ))}
          </select>
        )}
      </Setting>
      <Setting label={t('adv.lists.header.label')} plain={t('adv.lists.header.plain')}>
        {(described) => (
          <>
            <label htmlFor={lineId}>{t('adv.lists.header.field')}</label>{' '}
            <input
              id={lineId}
              type="number"
              min={1}
              className="adv-narrow"
              aria-describedby={described}
              value={sheet.read.headerLine}
              disabled={!draft.editable}
              onChange={(event) => {
                const line = Number(event.target.value);
                if (Number.isInteger(line) && line >= 1) {
                  read({ headerLine: line });
                }
              }}
            />
            {view === undefined || view.lines.length === 0 ? null : (
              <ol className="adv-lines">
                {view.lines.slice(0, 3).map((line, at) => (
                  <li key={at}>
                    <code>{line}</code>
                  </li>
                ))}
              </ol>
            )}
          </>
        )}
      </Setting>
      <Setting label={t('adv.lists.trim.label')} plain={t('adv.lists.trim.plain')}>
        {(described) => (
          <label className="adv-check">
            <input type="checkbox" aria-describedby={described} checked={sheet.read.trim} disabled={!draft.editable} onChange={(event) => { read({ trim: event.target.checked }); }} />{' '}
            {t('adv.lists.trim.field')}
          </label>
        )}
      </Setting>
    </>
  );
};

const Joining = ({ sheet, matches }: { readonly sheet: SheetPlan; readonly matches: SheetColumnMatches | undefined }): ReactElement => {
  const { t, number } = useAdvancedText();
  const draft = useDraft();
  const columns: readonly ColumnRef[] = matches?.columns.map(({ index, header }) => ({ index, header })) ?? (sheet.join === null ? [] : [sheet.join.column]);
  const current = sheet.join?.column.index;
  const normalize = sheet.join?.normalize ?? NO_NORMALIZE;
  return (
    <>
      <Setting label={t('adv.lists.join.label')} plain={t('adv.lists.join.plain')}>
        {(described) => (
          <select
            aria-label={t('adv.lists.join.label')}
            aria-describedby={described}
            value={current === undefined ? '' : String(current)}
            disabled={!draft.editable}
            onChange={(event) => {
              const column = columns.find((one) => String(one.index) === event.target.value) ?? null;
              draft.change((plan) => setJoinColumn(plan, sheet.id, column));
            }}
          >
            <option value="">{t('adv.lists.join.none')}</option>
            {columns.map((column) => {
              const match = matches?.columns.find((one) => one.index === column.index);
              return (
                <option key={column.index} value={String(column.index)}>
                  {match === undefined || matches === undefined ? column.header : matchWords(match, matches.rows, t, number)}
                </option>
              );
            })}
          </select>
        )}
      </Setting>
      {sheet.join === null ? null : (
        <>
          <Setting label={t('adv.lists.joinOn.label')} plain={t('adv.lists.joinOn.plain')}>
            {(described) => (
              <select
                aria-label={t('adv.lists.joinOn.label')}
                aria-describedby={described}
                value={sheet.join?.on}
                disabled={!draft.editable}
                onChange={(event) => {
                  const on = event.target.value === 'fileName' ? 'fileName' : 'identifier';
                  draft.change((plan) => setJoin(plan, sheet.id, { on }));
                }}
              >
                <option value="identifier">{t('adv.lists.joinOn.identifier')}</option>
                <option value="fileName">{t('adv.lists.joinOn.fileName')}</option>
              </select>
            )}
          </Setting>
          <Setting label={t('adv.lists.normalize.label')} plain={t('adv.lists.normalize.plain')}>
            {(described) => (
              <ul className="adv-list" aria-describedby={described}>
                {NORMALIZE.map(([key, words]) => (
                  <li key={key}>
                    <label className="adv-check">
                      <input
                        type="checkbox"
                        checked={normalize[key]}
                        disabled={!draft.editable}
                        onChange={(event) => {
                          const on = event.target.checked;
                          draft.change((plan) => setJoin(plan, sheet.id, { normalize: { ...normalize, [key]: on } }));
                        }}
                      />{' '}
                      {t(words)}
                    </label>
                  </li>
                ))}
              </ul>
            )}
          </Setting>
        </>
      )}
    </>
  );
};

const ROW_KINDS: readonly (readonly [string, AdvancedMessageKey])[] = [
  ['item', 'adv.lists.rowKind.item'],
  ['media', 'adv.lists.rowKind.media'],
  ['itemSet', 'adv.lists.rowKind.itemSet'],
  ['column', 'adv.lists.rowKind.column'],
];

const columnIn = (columns: readonly ColumnRef[], value: string): ColumnRef | null => columns.find((one) => String(one.index) === value) ?? null;

const RowKind = ({ sheet, matches }: { readonly sheet: SheetPlan; readonly matches: SheetColumnMatches | undefined }): ReactElement => {
  const { t } = useAdvancedText();
  const draft = useDraft();
  const columns: readonly ColumnRef[] = matches?.columns.map(({ index, header }) => ({ index, header })) ?? [];
  const kind = typeof sheet.rowKind === 'string' ? sheet.rowKind : 'column';
  const kindColumn = typeof sheet.rowKind === 'string' ? null : sheet.rowKind.column;
  const first = columns.find((column) => column.index !== sheet.join?.column.index) ?? columns[0];
  return (
    <>
      <Setting label={t('adv.lists.rowKind.label')} plain={t('adv.lists.rowKind.plain')}>
        {(described) => (
          <select
            aria-label={t('adv.lists.rowKind.label')}
            aria-describedby={described}
            value={kind}
            disabled={!draft.editable}
            onChange={(event) => {
              const value = event.target.value;
              if (value === 'item' || value === 'media' || value === 'itemSet') {
                draft.change((plan) => withRowKind(plan, sheet.id, value));
              } else if (first !== undefined) {
                draft.change((plan) => setRowKindColumn(plan, sheet.id, first));
              }
            }}
          >
            {ROW_KINDS.map(([value, words]) => (
              <option key={value} value={value} disabled={value === 'column' && first === undefined}>
                {t(words)}
              </option>
            ))}
          </select>
        )}
      </Setting>
      {kindColumn === null ? null : (
        <Setting label={t('adv.lists.rowKind.columnLabel')} plain={t('adv.lists.rowKind.columnPlain')}>
          {(described) => (
            <select
              aria-label={t('adv.lists.rowKind.columnLabel')}
              aria-describedby={described}
              value={String(kindColumn.index)}
              disabled={!draft.editable}
              onChange={(event) => {
                const column = columnIn(columns, event.target.value);
                if (column !== null) {
                  draft.change((plan) => setRowKindColumn(plan, sheet.id, column));
                }
              }}
            >
              {(columns.some((one) => one.index === kindColumn.index) ? columns : [kindColumn, ...columns]).map((column) => (
                <option key={column.index} value={String(column.index)}>
                  {column.header}
                </option>
              ))}
            </select>
          )}
        </Setting>
      )}
      {kind !== 'media' ? null : (
        <Setting label={t('adv.lists.itemKey.label')} plain={t('adv.lists.itemKey.plain')}>
          {(described) => (
            <select
              aria-label={t('adv.lists.itemKey.label')}
              aria-describedby={described}
              value={sheet.itemKey === null ? '' : String(sheet.itemKey.index)}
              disabled={!draft.editable}
              onChange={(event) => {
                const column = columnIn(columns, event.target.value);
                draft.change((plan) => setItemKey(plan, sheet.id, column));
              }}
            >
              <option value="">{t('adv.lists.itemKey.none')}</option>
              {columns.map((column) => (
                <option key={column.index} value={String(column.index)}>
                  {column.header}
                </option>
              ))}
            </select>
          )}
        </Setting>
      )}
    </>
  );
};

export const AdvancedLists = (): ReactElement => {
  const { t, number } = useAdvancedText();
  const draft = useDraft();
  const read = usePreview(['sheets', 'columnMatches', 'unpaired', 'listCandidates'], { unpaired: { rows: { perPage: 1 }, files: { perPage: 1 } } });
  const data = read.status === 'ready' ? read.data : null;
  const { lists } = seriesOf(draft.plan);
  return (
    <div className="adv-section" aria-busy={read.status === 'loading' || (read.status === 'ready' && read.busy)}>
      {read.status === 'error' ? <ErrorNotice error={read.error} /> : null}
      <ListPicker none={null} />
      {lists.length === 0 ? <p className="import-note">{t('adv.lists.none')}</p> : null}
      {lists.map((sheet) => {
        const view = data?.sheets.find((one) => one.id === sheet.id);
        const matches = data?.columnMatches.find((one) => one.sheet === sheet.id);
        const candidate = data?.listCandidates.find((one) => one.inPlan === sheet.id);
        const counts: ListCounts = { grain: candidate?.grain ?? null, matches, cells: candidate?.cells ?? [] };
        return (
          <section key={sheet.id} className="adv-sheet" aria-label={sheet.file}>
            <h4>
              {sheet.file} <code>{sheet.id}</code>
            </h4>
            {view?.error === null || view === undefined ? null : <p role="alert">{t('adv.lists.unread', { message: view.error })}</p>}
            {view?.read === null || view === undefined ? null : (
              <p className="adv-plain">{t('adv.lists.rows', { rows: number(view.read.rows), cells: number(view.read.cellsCount) })}</p>
            )}
            <Reading sheet={sheet} view={view} />
            <h5>{t('adv.lists.join.title')}</h5>
            <Joining sheet={sheet} matches={matches} />
            <RowKind sheet={sheet} matches={matches} />
            <h5>{t('adv.lists.columns.title')}</h5>
            <ColumnsTable sheet={sheet} view={view} counts={counts} />
          </section>
        );
      })}
      {data === null || lists.length === 0 ? null : (
        <Setting label={t('adv.lists.unpaired.label')} plain={t('adv.lists.unpaired.plain')}>
          {() => (
            <p>
              {countWords(data.unpaired.rows.total, ['adv.lists.unpaired.rowsNone', 'adv.lists.unpaired.rowsOne', 'adv.lists.unpaired.rows'], t, number)}{' '}
              {countWords(data.unpaired.items, ['adv.lists.unpaired.itemsNone', 'adv.lists.unpaired.itemsOne', 'adv.lists.unpaired.items'], t, number)}{' '}
              <a href={placeHref(draft.id, { mode: 'advanced', screen: 'pair' })}>{t('adv.lists.unpaired.pair')}</a>
            </p>
          )}
        </Setting>
      )}
      <h4>{t('adv.lists.roundTrip.title')}</h4>
      <Suspense fallback={null}>
        <RoundTrip />
      </Suspense>
    </div>
  );
};

export default AdvancedLists;
