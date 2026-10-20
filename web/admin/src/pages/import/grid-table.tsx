/**
 * One page of grid rows: the identifier pinned first, the row's problems
 * second, then the set, the type, the files, one button per value cell and
 * the spreadsheet columns no field takes. A cell with a problem names it in
 * words under the value, so the red is never the only sign. A row whose
 * identifier an item already holds says so in words and links to that item
 * instead of listing problems: the run never creates it.
 */
import type { ReactElement } from 'react';
import type { RowCell, RowPreview, RowsPreview } from '../../api/import-preview';
import { ROW_FIXES } from './grid-fixes';
import type { Fix } from './grid-fixes';
import { useImportText } from './messages';
import { CELL_PROBLEM, cellSource, columnSource } from './problem-text';

export interface CellRef {
  readonly identifier: string;
  readonly term: string;
}

/** Spaces at either end shown as middle dots, so a cell with them looks different. */
const showSpaces = (value: string): string => value.replace(/^ +| +$/g, (spaces) => '·'.repeat(spaces.length));

const baseName = (path: string): string => path.slice(path.lastIndexOf('/') + 1);

const Cell = ({
  cell,
  selected,
  onPick,
}: {
  readonly cell: RowCell;
  readonly selected: boolean;
  readonly onPick: () => void;
}): ReactElement => {
  const translator = useImportText();
  const { t } = translator;
  const problems = cell.problems.flatMap((kind) => {
    const key = CELL_PROBLEM[kind];
    return key === undefined ? [] : [t(key)];
  });
  const classes = ['import-cell', problems.length > 0 ? 'import-cell-bad' : '', selected ? 'import-cell-selected' : ''];
  return (
    <td className={classes.join(' ').trim()}>
      <button type="button" aria-pressed={selected} title={cellSource(translator, cell.source)} onClick={onPick}>
        {cell.value === '' ? <span className="query-count">{t('grid.empty')}</span> : showSpaces(cell.value)}
        {cell.conflict === undefined ? null : (
          <span className="import-conflict" title={t('grid.disagree')}>
            {' ≠'}
            <span className="import-hidden">{t('grid.disagree')}</span>
          </span>
        )}
        {problems.length === 0 ? null : <small className="import-problem">{problems.join(' · ')}</small>}
      </button>
    </td>
  );
};

const Existing = ({ itemId }: { readonly itemId: number }): ReactElement => {
  const { t } = useImportText();
  return (
    <td className="import-row-problems">
      <span className="query-count">{t('grid.existing')} </span>
      <a href={`#/items/${String(itemId)}`}>{t('grid.openItem', { id: String(itemId) })}</a>
    </td>
  );
};

const RowProblems = ({
  row,
  editable,
  onRowFix,
  onSkip,
}: {
  readonly row: RowPreview;
  readonly editable: boolean;
  readonly onRowFix: (fix: Fix, identifier: string) => void;
  readonly onSkip: (identifier: string, skip: boolean) => void;
}): ReactElement => {
  const { t, number } = useImportText();
  const cellProblems = row.problems - row.rowProblems.length;
  return (
    <td className="import-row-problems">
      {row.rowProblems.length === 0 ? null : (
        <ul>
          {row.rowProblems.map((problem) => (
            <li key={problem.kind}>
              {problem.kind === 'cells'
                ? t('problem.cells', { line: problem.line ?? 0, cells: problem.cells ?? 0 })
                : t('problem.nofile')}{' '}
              {ROW_FIXES[problem.kind].map((fix) => (
                <button
                  key={fix.label}
                  type="button"
                  className="import-link"
                  disabled={!editable}
                  onClick={() => { onRowFix(fix, row.identifier); }}
                >
                  {t(fix.label)}
                </button>
              ))}
            </li>
          ))}
        </ul>
      )}
      {cellProblems > 0 ? <span className="query-count">{t('grid.cellsToFix', { count: number(cellProblems) })} </span> : null}
      {row.skipped ? <span className="query-count">{t('grid.skipped')} </span> : null}
      {row.skipped || row.problems > 0 ? (
        <button
          type="button"
          className="import-link"
          disabled={!editable}
          onClick={() => { onSkip(row.identifier, !row.skipped); }}
        >
          {t(row.skipped ? 'grid.includeRow' : 'grid.skipRow')}
        </button>
      ) : null}
    </td>
  );
};

export interface GridTableProps {
  readonly rows: RowsPreview;
  readonly selected: CellRef | null;
  readonly editable: boolean;
  readonly label: (term: string) => string;
  readonly onPick: (cell: CellRef) => void;
  readonly onFill: (term: string) => void;
  readonly onMap: (header: string) => void;
  readonly onRowFix: (fix: Fix, identifier: string) => void;
  readonly onSkip: (identifier: string, skip: boolean) => void;
}

const Head = ({ rows, editable, label, onFill, onMap }: GridTableProps): ReactElement => {
  const translator = useImportText();
  const { t } = translator;
  return (
    <thead>
      <tr>
        <th className="import-pinned">
          {t('grid.identifier')}
          <small>{t('grid.fromName')}</small>
        </th>
        <th>{t('grid.problems')}</th>
        <th>{t('grid.set')}</th>
        <th>{t('grid.type')}</th>
        <th>{t('grid.files')}</th>
        {rows.columns.map((column) => (
          <th key={column.term}>
            {label(column.term)}
            <small className="import-mono">{column.term}</small>
            <small>{column.sources.map((source) => columnSource(translator, source)).join(' · ')}</small>
            <button type="button" className="import-link" disabled={!editable} onClick={() => { onFill(column.term); }}>
              {t('grid.fillEmpty')}
            </button>
          </th>
        ))}
        {rows.unmapped.map((header) => (
          <th key={`csv:${header}`}>
            {header}
            <small>{t('grid.notMapped')}</small>
            <button type="button" className="import-link" disabled={!editable} onClick={() => { onMap(header); }}>
              {t('grid.map')}
            </button>
          </th>
        ))}
      </tr>
    </thead>
  );
};

export const GridTable = (props: GridTableProps): ReactElement => {
  const { t } = useImportText();
  const { rows, selected, editable, onPick, onRowFix, onSkip } = props;
  return (
    <div className="table-scroll import-grid">
      <table>
        <Head {...props} />
        <tbody>
          {rows.page.map((row) => (
            <tr key={row.identifier} className={row.skipped || row.existing !== null ? 'import-skipped' : undefined}>
              <th scope="row" className="import-pinned import-mono">
                {row.identifier}
              </th>
              {row.existing === null ? (
                <RowProblems row={row} editable={editable} onRowFix={onRowFix} onSkip={onSkip} />
              ) : (
                <Existing itemId={row.existing.itemId} />
              )}
              <td>{row.sets.length === 0 ? <span className="query-count">{t('grid.noSet')}</span> : row.sets.join(', ')}</td>
              <td>{row.type}</td>
              <td>
                {row.files.length}{' '}
                <span className="query-count">{row.files.map(baseName).join(', ')}</span>
              </td>
              {row.cells.map((cell) => (
                <Cell
                  key={cell.term}
                  cell={cell}
                  selected={selected?.identifier === row.identifier && selected.term === cell.term}
                  onPick={() => { onPick({ identifier: row.identifier, term: cell.term }); }}
                />
              ))}
              {row.extra.map((value, index) => (
                <td key={`csv:${rows.unmapped[index] ?? String(index)}`} className="import-unmapped">
                  {value}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
};
