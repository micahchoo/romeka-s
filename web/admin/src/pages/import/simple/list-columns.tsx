/**
 * "Where each column goes": one row per column of the list but the pairing
 * column, with its first value and the field it goes into. A column goes into
 * the field with the same name; every other column says that no field has its
 * name, and stays out until the person chooses. When each row describes one
 * file, a column's values go on the item, on each file, or nowhere, with in
 * how many items they differ and the place that count suggests. A column
 * whose cells hold a separator offers to cut them into several values.
 */
import type { ReactElement } from 'react';
import type { ColumnRef, SheetPlan } from '../../../api/import-plan';
import type { Grain } from '../../../api/import-sections';
import type { SheetView } from '../../../api/import-sheet-views';
import { TableScroll } from '../../../app/table-scroll';
import { useAsync } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { loadProperties } from '../lists/catalog';
import { asTarget, fileLevelAllowed, filledOf, firstRowOf, fold, levelOf, separatorOf, splitOf, suggestedLevel, targetOf, variesOf, withColumnLevel, withCountedTarget, withSplit } from './list-plan';
import type { Level, ListCounts } from './list-plan';
import { useSimpleText } from './messages';
import type { SimpleMessageKey } from './messages';

/** Simple offers the Dublin Core fields; Advanced offers every field of Omeka. */
const SHOWN_VOCABULARY = 'dcterms:';

const LEVELS: readonly (readonly [Level, SimpleMessageKey])[] = [
  ['item', 'guided.list.levelItem'],
  ['file', 'guided.list.levelFile'],
  ['out', 'guided.list.levelOut'],
];

const LevelChoice = ({ sheet, column, grain }: { readonly sheet: SheetPlan; readonly column: ColumnRef; readonly grain: Grain | null }): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const level = levelOf(draft.plan, sheet.id, column.index);
  const fileAllowed = fileLevelAllowed(draft.plan, sheet.id, column.index);
  const varies = variesOf(grain, column.index);
  const suggested = suggestedLevel(draft.plan, sheet.id, column.index);
  return (
    <td>
      <fieldset className="guided-opts" aria-label={t('guided.list.levelOf', { column: column.header })}>
        {LEVELS.map(([value, label]) => (
          <label key={value}>
            <input
              type="radio"
              name={`level-${sheet.id}-${String(column.index)}`}
              value={value}
              checked={level === value}
              disabled={!draft.editable || (value !== 'out' && level === 'out') || (value === 'file' && !fileAllowed)}
              onChange={() => {
                draft.change((plan) => withColumnLevel(plan, sheet.id, column, value));
              }}
            />
            <span>
              {t(label)}
              {value === suggested && level !== 'out' ? <span className="guided-pill">{t('guided.check.suggested')}</span> : null}
            </span>
          </label>
        ))}
      </fieldset>
      {varies === null ? null : <div className="muted">{t('guided.list.varies', { count: number(varies.groups), of: number(varies.of) })}</div>}
      {fileAllowed ? null : <div className="muted">{t('guided.list.itemOnly')}</div>}
    </td>
  );
};

const SplitChoice = ({ sheet, column, counts }: { readonly sheet: SheetPlan; readonly column: ColumnRef; readonly counts: ListCounts }): ReactElement | null => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const separator = separatorOf(counts, column.index);
  const filled = filledOf(counts, column.index);
  if (separator === null || filled === null) {
    return null;
  }
  return (
    <div>
      <label>
        <input
          type="checkbox"
          checked={splitOf(draft.plan, sheet.id, column.index) === separator.text}
          disabled={!draft.editable}
          onChange={(event) => {
            const split = event.target.checked ? separator.text : null;
            draft.change((plan) => withSplit(plan, sheet.id, column, split));
          }}
        />{' '}
        {t('guided.list.split', { separator: separator.text })}
      </label>
      <div className="muted">{t('guided.list.splitCount', { count: number(separator.cells), of: number(filled) })}</div>
    </div>
  );
};

export const ListColumns = ({ sheet, view, counts }: { readonly sheet: SheetPlan; readonly view: SheetView; readonly counts: ListCounts }): ReactElement => {
  const { t } = useSimpleText();
  const grain = counts.grain;
  const firstRow = firstRowOf(view, sheet.read.headerLine);
  const draft = useDraft();
  const properties = useAsync(loadProperties, []);
  const fields = properties.status === 'ready' ? properties.data.filter((property) => property.term.startsWith(SHOWN_VOCABULARY)) : [];
  const columns = sheet.columns.filter((column) => sheet.join === null || column.column.index !== sheet.join.column.index);
  const perFile = sheet.rowKind === 'media';
  return (
    <>
      <h4>{t('guided.list.columnsTitle')}</h4>
      <p className="muted">{t('guided.list.columnsLede')}</p>
      <TableScroll label={t('guided.list.columnsTable')}>
        <table>
          <thead>
            <tr>
              <th scope="col">{t('guided.list.colColumn')}</th>
              <th scope="col">{t('guided.list.colFirst')}</th>
              <th scope="col">{t('guided.list.colGoes')}</th>
              {perFile ? <th scope="col">{t('guided.list.colLevel')}</th> : null}
            </tr>
          </thead>
          <tbody>
            {columns.map(({ column }) => {
              const target = targetOf(draft.plan, sheet.id, column.index);
              const same = fields.find((field) => field.label !== null && fold(field.label) === fold(column.header));
              const shown = fields.some((field) => `field:${field.term}` === target) || !target.startsWith('field:');
              const note =
                target === ''
                  ? t('guided.list.noField')
                  : same !== undefined && target === `field:${same.term}`
                    ? t('guided.list.sameName')
                    : null;
              return (
                <tr key={column.index} data-decision={`column:${column.header}`}>
                  <th scope="row">{column.header}</th>
                  <td className="muted">{(firstRow[column.index] ?? '').trim()}</td>
                  <td>
                    <select
                      aria-label={t('guided.list.goesInto', { column: column.header })}
                      value={target}
                      disabled={!draft.editable}
                      onChange={(event) => {
                        const next = asTarget(event.target.value);
                        if (next !== null) {
                          draft.change((plan) => withCountedTarget(plan, sheet.id, column, next, counts));
                        }
                      }}
                    >
                      <option value="">{t('guided.list.notAdd')}</option>
                      <optgroup label={t('guided.list.fields')}>
                        {shown ? null : <option value={target}>{target.slice('field:'.length)}</option>}
                        {fields.map((field) => (
                          <option key={field.term} value={`field:${field.term}`}>
                            {field.label ?? field.term}
                          </option>
                        ))}
                      </optgroup>
                      <option value="template">{t('guided.list.toType')}</option>
                      <option value="sets">{t('guided.list.toSet')}</option>
                      <option value={`new:${column.header}`}>{t('guided.list.newField', { name: column.header })}</option>
                    </select>
                    {note === null ? null : <div className="muted">{note}</div>}
                    {target === '' ? null : <SplitChoice sheet={sheet} column={column} counts={counts} />}
                  </td>
                  {perFile ? <LevelChoice sheet={sheet} column={column} grain={grain} /> : null}
                </tr>
              );
            })}
          </tbody>
        </table>
      </TableScroll>
      {perFile ? <p className="muted">{t('guided.list.levelLede')}</p> : null}
      <p className="muted">{t('guided.list.moreFields')}</p>
    </>
  );
};
