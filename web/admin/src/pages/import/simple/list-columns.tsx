/**
 * "Where each column goes": one row per column of the list as read but the
 * pairing column, with its first value under its name and the field it goes into. A
 * column goes into the field with the same name; every other column stays
 * out until the person chooses. When each row describes one file, a column's
 * values go on the item or on each file: the proposer's pick is marked, each
 * answer says how many items or files it gives values, and the row says in
 * how many items they differ. A column whose cells hold a
 * separator offers to cut them into several values.
 */
import type { ReactElement } from 'react';
import type { ColumnRef, SheetPlan } from '../../../api/import-plan';
import type { RollUpCounts } from '../../../api/import-proposal';
import type { Grain } from '../../../api/import-sections';
import type { SheetView } from '../../../api/import-sheet-views';
import { TableScroll } from '../../../app/table-scroll';
import { useAsync } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { loadProperties } from '../lists/catalog';
import { asTarget, fileLevelAllowed, filledOf, firstRowOf, levelOf, rollUpOf, separatorOf, splitOf, suggestedLevel, targetOf, variesOf, withColumnLevel, withCountedTarget, withSplit } from './list-plan';
import type { Level, ListCounts } from './list-plan';
import { Segmented } from './master-detail';
import { useSimpleText } from './messages';

/** Simple offers the Dublin Core fields; Advanced offers every field of Omeka. */
const SHOWN_VOCABULARY = 'dcterms:';

/** On the item or on each file: the proposer's pick carries the ✓, and each answer shows what it gives. */
const LevelChoice = ({ sheet, column, counts }: { readonly sheet: SheetPlan; readonly column: ColumnRef; readonly counts: ListCounts }): ReactElement | null => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const level = levelOf(draft.plan, sheet.id, column.index);
  if (level === 'out') {
    return null;
  }
  const suggested = suggestedLevel(draft.plan, sheet.id, column, counts.rollUps);
  const pick = rollUpOf(counts.rollUps, column)?.pick;
  const given = (value: Exclude<Level, 'out'>): RollUpCounts | undefined => [pick?.chosen, pick?.rival].find((one) => one?.value === value)?.counts;
  const note = (value: Exclude<Level, 'out'>): { readonly note?: string } => {
    const made = given(value);
    if (made === undefined) {
      return {};
    }
    return { note: value === 'item' ? t('guided.list.levelItems', { count: number(made.items) }) : t('guided.list.levelFiles', { count: number(made.files) }) };
  };
  const levels: readonly Exclude<Level, 'out'>[] = fileLevelAllowed(draft.plan, sheet.id, column.index) ? ['item', 'file'] : ['item'];
  return (
    <Segmented
      label={t('guided.list.levelOf', { column: column.header })}
      segments={levels.map((value) => ({ value, label: t(value === 'item' ? 'guided.list.kindItem' : 'guided.list.levelFile'), suggested: value === suggested, ...note(value) }))}
      value={level}
      disabled={!draft.editable}
      suggestedWord={t('guided.check.suggested')}
      onChange={(value) => {
        draft.change((plan) => withColumnLevel(plan, sheet.id, column, value));
      }}
    />
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
    <label className="guided-tgl">
      <input
        type="checkbox"
        checked={splitOf(draft.plan, sheet.id, column.index) === separator.text}
        disabled={!draft.editable}
        onChange={(event) => {
          const split = event.target.checked ? separator.text : null;
          draft.change((plan) => withSplit(plan, sheet.id, column, split));
        }}
      />
      {t('guided.list.splitAt', { separator: separator.text })}
      <span className="muted">
        {number(separator.cells)}/{number(filled)}
      </span>
    </label>
  );
};

const Differs = ({ grain, column }: { readonly grain: Grain | null; readonly column: ColumnRef }): ReactElement | null => {
  const { t, number } = useSimpleText();
  const varies = variesOf(grain, column.index);
  return varies === null ? null : (
    <span className={varies.groups > 0 ? 'guided-chip is-warn' : 'guided-chip'}>{t('guided.list.differs', { count: number(varies.groups), of: number(varies.of) })}</span>
  );
};

export const ListColumns = ({ sheet, view, counts }: { readonly sheet: SheetPlan; readonly view: SheetView; readonly counts: ListCounts }): ReactElement => {
  const { t } = useSimpleText();
  const firstRow = firstRowOf(view, sheet.read.headerLine);
  const draft = useDraft();
  const properties = useAsync(loadProperties, []);
  const fields = properties.status === 'ready' ? properties.data.filter((property) => property.term.startsWith(SHOWN_VOCABULARY)) : [];
  // The file's own columns: a list the proposer chose holds no column of the plan until one is mapped.
  const read = view.read?.columns.filter((column) => column.mappable) ?? sheet.columns.map((one) => one.column);
  const columns = read.filter((column) => sheet.join === null || column.index !== sheet.join.column.index);
  const perFile = sheet.rowKind === 'media';
  return (
    <>
      <h4>{t('guided.list.columnsTitle')}</h4>
      <TableScroll label={t('guided.list.columnsTable')}>
        <table className="guided-columns">
          <thead>
            <tr>
              <th scope="col">{t('guided.list.colColumn')}</th>
              <th scope="col">{t('guided.list.colGoes')}</th>
              {perFile ? <th scope="col">{t('guided.list.colOn')}</th> : null}
              <th scope="col">{t('guided.list.colSplit')}</th>
            </tr>
          </thead>
          <tbody>
            {columns.map(({ index, header }) => {
              const column = { index, header };
              const target = targetOf(draft.plan, sheet.id, column.index);
              const shown = fields.some((field) => `field:${field.term}` === target) || !target.startsWith('field:');
              return (
                <tr key={column.index} data-decision={`column:${column.header}`} className={target === '' ? 'is-off' : undefined}>
                  <th scope="row">
                    <span className="guided-col-name">{column.header}</span>{' '}
                    <span className="guided-first muted">{(firstRow[column.index] ?? '').trim()}</span>
                  </th>
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
                  </td>
                  {perFile ? (
                    <td>
                      <span className="guided-on">
                        <LevelChoice sheet={sheet} column={column} counts={counts} />
                        {target === '' ? null : <Differs grain={counts.grain} column={column} />}
                      </span>
                    </td>
                  ) : null}
                  <td>{target === '' ? null : <SplitChoice sheet={sheet} column={column} counts={counts} />}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </TableScroll>
    </>
  );
};
