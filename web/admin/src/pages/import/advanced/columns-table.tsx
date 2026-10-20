/**
 * One list's columns: the field each feeds and how its cells become values
 * (split, type, language, private). A column feeds a field when the field's
 * rule reads it, so the Field choice here and the sources under Fields are
 * one setting seen from two sides.
 */
import type { ReactElement } from 'react';
import type { ColumnRef, SheetPlan, ValueType } from '../../../api/import-plan';
import type { SheetView } from '../../../api/import-sheet-views';
import { TableScroll } from '../../../app/table-scroll';
import { useDraft } from '../draft-context';
import { fieldOptions, optionFor, slotWords, useProperties } from './field-names';
import { useAdvancedText } from './messages';
import type { AdvancedTranslator } from './messages';
import { columnOptions, columnTarget, mapColumn, setColumnOptions } from './plan-edits';

interface Column extends ColumnRef {
  readonly mappable: boolean;
}

const columnsOf = (sheet: SheetPlan, view: SheetView | undefined): readonly Column[] => {
  if (view?.read != null) {
    return view.read.columns;
  }
  const known = [...(sheet.join === null ? [] : [sheet.join.column]), ...sheet.columns.map((one) => one.column)];
  return known
    .filter((column, at) => known.findIndex((other) => other.index === column.index) === at)
    .sort((a, b) => a.index - b.index)
    .map((column) => ({ ...column, mappable: true }));
};

const typeValue = (type: ValueType): string => {
  if (type === 'literal' || type === 'uri') {
    return type;
  }
  return 'resource' in type ? `resource:${type.resource}` : `dataType:${type.dataType}`;
};

const typeOf = (value: string): ValueType => {
  if (value === 'uri') {
    return 'uri';
  }
  if (value === 'resource:identifier' || value === 'resource:internalId') {
    return { resource: value === 'resource:identifier' ? 'identifier' : 'internalId' };
  }
  return value.startsWith('dataType:') ? { dataType: value.slice('dataType:'.length) } : 'literal';
};

const typeOptions = (current: ValueType, t: AdvancedTranslator['t']): readonly (readonly [string, string])[] => [
  ['literal', t('adv.lists.type.literal')],
  ['uri', t('adv.lists.type.uri')],
  ['resource:identifier', t('adv.lists.type.identifier')],
  ['resource:internalId', t('adv.lists.type.internalId')],
  ...(typeof current === 'object' && 'dataType' in current ? [[typeValue(current), t('adv.lists.type.dataType', { name: current.dataType })] as const] : []),
];

/** Empty text is no value: `null` in the plan. */
const textOrNull = (value: string): string | null => (value === '' ? null : value);

export const ColumnsTable = ({ sheet, view }: { readonly sheet: SheetPlan; readonly view: SheetView | undefined }): ReactElement => {
  const { t } = useAdvancedText();
  const draft = useDraft();
  const properties = useProperties();
  const known = properties.status === 'ready' ? properties.data : [];
  const options = fieldOptions(['sets', 'template', 'visibility'], known, t);
  const columns = columnsOf(sheet, view);
  return (
    <TableScroll label={t('adv.lists.columns.label', { file: sheet.file })}>
      <p className="adv-plain">{t('adv.lists.columns.plain')}</p>
      <table className="adv-columns">
        <thead>
          <tr>
            <th scope="col">{t('adv.lists.columns.column')}</th>
            <th scope="col">{t('adv.lists.columns.field')}</th>
            <th scope="col">{t('adv.lists.columns.split')}</th>
            <th scope="col">{t('adv.lists.columns.type')}</th>
            <th scope="col">{t('adv.lists.columns.lang')}</th>
            <th scope="col">{t('adv.lists.columns.private')}</th>
          </tr>
        </thead>
        <tbody>
          {columns.map((column) => {
            const ref = { index: column.index, header: column.header };
            const target = columnTarget(draft.plan, sheet.id, ref);
            const shown = target === null || options.some((option) => option.value === target) ? options : [...options, { value: target, label: slotWords(target, known, t) }];
            const own = columnOptions(sheet, ref);
            const name = column.header === '' ? t('adv.lists.columns.unnamed', { index: column.index + 1 }) : column.header;
            const joined = sheet.join?.column.index === column.index;
            const locked = !draft.editable || !column.mappable;
            const set = (patch: Parameters<typeof setColumnOptions>[3]): void => {
              draft.change((plan) => setColumnOptions(plan, sheet.id, ref, patch));
            };
            return (
              <tr key={column.index}>
                <th scope="row">
                  {name} {joined ? <span className="import-pill">{t('adv.lists.columns.joined')}</span> : null}
                  {column.mappable ? null : <p className="adv-plain">{t('adv.lists.columns.notMappable')}</p>}
                </th>
                <td>
                  <select
                    aria-label={t('adv.lists.columns.fieldOf', { column: name })}
                    value={target ?? ''}
                    disabled={locked}
                    onChange={(event) => {
                      const slot = optionFor(shown, event.target.value);
                      draft.change((plan) => mapColumn(plan, sheet.id, ref, slot));
                    }}
                  >
                    <option value="">{t('adv.lists.columns.unmapped')}</option>
                    {shown.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                </td>
                <td>
                  <input
                    type="text"
                    className="adv-narrow"
                    aria-label={t('adv.lists.columns.splitOf', { column: name })}
                    value={own.split ?? ''}
                    disabled={locked}
                    onChange={(event) => {
                      set({ split: textOrNull(event.target.value) });
                    }}
                  />
                </td>
                <td>
                  <select aria-label={t('adv.lists.columns.typeOf', { column: name })} value={typeValue(own.type)} disabled={locked} onChange={(event) => { set({ type: typeOf(event.target.value) }); }}>
                    {typeOptions(own.type, t).map(([value, label]) => (
                      <option key={value} value={value}>
                        {label}
                      </option>
                    ))}
                  </select>
                </td>
                <td>
                  <input
                    type="text"
                    className="adv-narrow"
                    aria-label={t('adv.lists.columns.langOf', { column: name })}
                    value={own.lang ?? ''}
                    disabled={locked}
                    onChange={(event) => {
                      set({ lang: textOrNull(event.target.value.trim()) });
                    }}
                  />
                </td>
                <td>
                  <input type="checkbox" aria-label={t('adv.lists.columns.privateOf', { column: name })} checked={own.private} disabled={locked} onChange={(event) => { set({ private: event.target.checked }); }} />
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </TableScroll>
  );
};
