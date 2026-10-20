/**
 * "Where each column goes": one row per column of the list but the pairing
 * column, with its first value and the field it goes into. A column goes into
 * the field with the same name; every other column says that no field has its
 * name, and stays out until the person chooses.
 */
import type { ReactElement } from 'react';
import type { SheetPlan } from '../../../api/import-plan';
import type { SheetView } from '../../../api/import-sheet-views';
import { TableScroll } from '../../../app/table-scroll';
import { useAsync } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { loadProperties } from '../lists/catalog';
import { asTarget, cellsOf, fold, targetOf, withTarget } from './list-plan';
import { useSimpleText } from './messages';

/** Simple offers the Dublin Core fields; Advanced offers every field of Omeka. */
const SHOWN_VOCABULARY = 'dcterms:';

export const ListColumns = ({ sheet, view }: { readonly sheet: SheetPlan; readonly view: SheetView }): ReactElement => {
  const { t } = useSimpleText();
  const draft = useDraft();
  const properties = useAsync(loadProperties, []);
  const fields = properties.status === 'ready' ? properties.data.filter((property) => property.term.startsWith(SHOWN_VOCABULARY)) : [];
  const delimiter = view.read?.delimiter ?? ',';
  const firstRow = cellsOf(view.lines[sheet.read.headerLine] ?? '', delimiter === 'auto' ? ',' : delimiter);
  const columns = sheet.columns.filter((column) => sheet.join === null || column.column.index !== sheet.join.column.index);
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
                          draft.change((plan) => withTarget(plan, sheet.id, column, next));
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
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </TableScroll>
      <p className="muted">{t('guided.list.moreFields')}</p>
    </>
  );
};
