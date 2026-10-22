/**
 * One list's columns: the field each feeds and how its cells become values
 * (split, type, language, private). A column feeds a field when the field's
 * rule reads it, so the Field choice here and the sources under Fields are
 * one setting seen from two sides. When each row describes a file, a column
 * mapped to a field goes on the item (`slots.item`), on each file
 * (`slots.media`) or nowhere, read from where its sources are. A column's
 * first field puts it where its count suggests, split as its cells suggest.
 */
import type { ReactElement } from 'react';
import type { ColumnRef, SheetPlan, ValueType } from '../../../api/import-plan';
import type { Grain } from '../../../api/import-sections';
import type { SheetView } from '../../../api/import-sheet-views';
import { TableScroll } from '../../../app/table-scroll';
import { useDraft } from '../draft-context';
import { fileLevelAllowed, levelOf, suggestedLevel, variesOf, withColumnLevel, withCounted } from '../simple/list-plan';
import type { Level, ListCounts } from '../simple/list-plan';
import { fieldOptions, optionFor, slotWords, useProperties } from './field-names';
import { useAdvancedText } from './messages';
import type { AdvancedMessageKey, AdvancedTranslator } from './messages';
import { columnOptions, columnTarget, mapColumn, setColumnOptions } from './plan-edits';
import { written } from './writes';
import { seriesHolding } from '../series';

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

const LEVELS: readonly Level[] = ['item', 'file', 'out'];

const levelIn = (value: string): Level | null => LEVELS.find((one) => one === value) ?? null;

/** Empty text is no value: `null` in the plan. */
const textOrNull = (value: string): string | null => (value === '' ? null : value);

export const ColumnsTable = ({ sheet, view, counts }: { readonly sheet: SheetPlan; readonly view: SheetView | undefined; readonly counts: ListCounts }): ReactElement => {
  const { t, number } = useAdvancedText();
  const draft = useDraft();
  const properties = useProperties();
  const known = properties.status === 'ready' ? properties.data : [];
  const options = fieldOptions(['sets', 'template', 'visibility'], known, t);
  const columns = columnsOf(sheet, view);
  const perFile = sheet.rowKind === 'media';
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
            {perFile ? <th scope="col">{t('adv.lists.columns.level')}</th> : null}
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
                      draft.change((plan) => withCounted(plan, mapColumn(plan, sheet.id, ref, slot), sheet.id, ref, counts));
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
                {perFile ? <LevelCell sheet={sheet} column={ref} name={name} grain={counts.grain} locked={locked} number={number} /> : null}
              </tr>
            );
          })}
        </tbody>
      </table>
    </TableScroll>
  );
};

const LEVEL_WORDS: Readonly<Record<Level, AdvancedMessageKey>> = {
  item: 'adv.lists.columns.levelItem',
  file: 'adv.lists.columns.levelFile',
  out: 'adv.lists.columns.levelOut',
};

const LevelCell = ({
  sheet,
  column,
  name,
  grain,
  locked,
  number,
}: {
  readonly sheet: SheetPlan;
  readonly column: ColumnRef;
  readonly name: string;
  readonly grain: Grain | null;
  readonly locked: boolean;
  readonly number: (n: number) => string;
}): ReactElement => {
  const { t } = useAdvancedText();
  const draft = useDraft();
  const level = levelOf(draft.plan, sheet.id, column.index);
  const fileAllowed = fileLevelAllowed(draft.plan, sheet.id, column.index);
  const varies = variesOf(grain, column.index);
  return (
    <td>
      <select
        aria-label={t('adv.lists.columns.levelOf', { column: name })}
        value={level}
        disabled={locked || level === 'out'}
        onChange={(event) => {
          const next = levelIn(event.target.value);
          if (next !== null) {
            draft.change((plan) => written(plan, 'columnLevel', seriesHolding(plan, sheet.id)?.id, (now) => withColumnLevel(now, sheet.id, column, next)));
          }
        }}
      >
        {LEVELS.map((one) => (
          <option key={one} value={one} disabled={one === 'file' && !fileAllowed}>
            {t(LEVEL_WORDS[one])}
          </option>
        ))}
      </select>
      {varies === null ? null : (
        <p className="adv-plain">
          {t('adv.lists.columns.varies', { count: number(varies.groups), of: number(varies.of) })}{' '}
          {level === 'out' ? null : t(suggestedLevel(draft.plan, sheet.id, column.index) === 'file' ? 'adv.lists.columns.suggestFile' : 'adv.lists.columns.suggestItem')}
        </p>
      )}
    </td>
  );
};
