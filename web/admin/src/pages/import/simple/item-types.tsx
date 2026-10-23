/**
 * Where a folder's item type comes from: nowhere, a column of one of its
 * lists, or the top folder; then each value and the item type it becomes
 * (`../value-map.tsx`). A value is set to an item type only when an item
 * type in Omeka has the same name; every other value says it is not chosen,
 * and the person maps it.
 *
 * A column is offered as the list's file reads it, so a list the proposer
 * chose, which maps no column yet, offers its columns too; taking one maps
 * it in its list, because the server refuses a rule that reads a column the
 * list does not map. The rule is a carried member of a suggested folder:
 * the proposer copies it into its next proposal, so it survives every click.
 *
 * Structure's folder node draws it in the plan as the tree shows it
 * (`structure-list.tsx`, `ItemTypesHome`); "Where they go" draws it on the
 * plan's one series. A chunk of its own.
 */
import { Suspense, lazy } from 'react';
import type { ReactElement } from 'react';
import type { ColumnRef, ImportPlan, LevelId, SlotRule } from '../../../api/import-plan';
import { useDraft } from '../draft-context';
import { usePreview } from '../lists/use-preview';
import { seriesOf, shownPlanFor, withSeries } from '../series';
import { withColumnMapped } from './list-plan';
import { useSimpleText } from './messages';

const ValueMap = lazy(() => import('../value-map'));

/** Where the item type of a series comes from: nowhere, a list's column (`sheet` and the column), or the top folder. */
type TypeFrom = { readonly from: 'none' } | { readonly from: 'top' } | { readonly from: 'column'; readonly sheet: string; readonly column: ColumnRef };

const NONE: SlotRule = { sources: [], combine: 'first' };

const ruleOf = (plan: ImportPlan, level: LevelId): SlotRule => seriesOf(plan, level).slots.item.template ?? NONE;

const typeFromOf = (plan: ImportPlan, level: LevelId): TypeFrom => {
  const first = ruleOf(plan, level).sources[0];
  if (first?.from === 'column') {
    return { from: 'column', sheet: first.sheet, column: first.column };
  }
  return first?.from === 'folder' ? { from: 'top' } : { from: 'none' };
};

/** The item type rule of `level` reading `from`; a rule with no source and no map is absent. */
const withTypeFrom = (plan: ImportPlan, level: LevelId, from: TypeFrom): ImportPlan => {
  const rule = { ...ruleOf(plan, level), sources: from.from === 'column' ? [{ from: 'column' as const, sheet: from.sheet, column: from.column }] : from.from === 'top' ? [{ from: 'folder' as const, level: 1 }] : [] };
  const kept = rule.sources.length === 0 && (rule.map === undefined || Object.keys(rule.map).length === 0) ? undefined : rule;
  const mapped = from.from === 'column' ? withColumnMapped(plan, from.sheet, from.column) : plan;
  return withSeries(mapped, level, (series) => {
    const item = Object.fromEntries(Object.entries(series.slots.item).filter(([slot]) => slot !== 'template'));
    return { ...series, slots: { ...series.slots, item: kept === undefined ? item : { ...item, template: kept } } };
  });
};

const keyOf = (from: TypeFrom): string => (from.from === 'column' ? `column:${from.sheet}:${String(from.column.index)}` : from.from);

export const ItemTypes = ({ level }: { readonly level: LevelId }): ReactElement => {
  const { t } = useSimpleText();
  const draft = useDraft();
  const read = usePreview(['sheets'], shownPlanFor(draft, level));
  const from = typeFromOf(draft.plan, level);
  const { lists } = seriesOf(draft.plan, level);
  // The file's own columns once read, else the ones its list maps.
  const columns = lists.flatMap((sheet) => {
    const view = read.status === 'ready' ? read.data.sheets.find((one) => one.id === sheet.id) : undefined;
    const known = view?.read?.columns.filter((column) => column.mappable).map(({ index, header }) => ({ index, header })) ?? sheet.columns.map((one) => one.column);
    return known.map((column) => ({ from: 'column' as const, sheet: sheet.id, file: sheet.file, column }));
  });
  const several = lists.length > 1;
  return (
    <section className="guided-inspector guided-inspect" aria-label={t('guided.where.typesTitle')}>
      <h4>{t('guided.where.typesTitle')}</h4>
      <dl className="guided-kv">
        <dt>{t('guided.where.from')}</dt>
        <dd data-decision="type-from">
          <select
            aria-label={t('guided.where.typeFrom')}
            value={keyOf(from)}
            disabled={!draft.editable}
            onChange={(event) => {
              const value = event.target.value;
              const next: TypeFrom = columns.find((one) => keyOf(one) === value) ?? (value === 'top' ? { from: 'top' } : { from: 'none' });
              draft.change((plan) => withTypeFrom(plan, level, next));
            }}
          >
            <option value="none">{t('guided.where.fromNone')}</option>
            {from.from === 'column' && !columns.some((one) => keyOf(one) === keyOf(from)) ? <option value={keyOf(from)}>{t('guided.where.fromColumn', { column: from.column.header })}</option> : null}
            {columns.map((one) => (
              <option key={keyOf(one)} value={keyOf(one)}>
                {several ? t('guided.where.fromColumnOf', { column: one.column.header, file: one.file }) : t('guided.where.fromColumn', { column: one.column.header })}
              </option>
            ))}
            <option value="top">{t('guided.where.fromTop')}</option>
          </select>
        </dd>
      </dl>
      {from.from === 'none' ? null : (
        <Suspense fallback={null}>
          <ValueMap home={{ level, kind: 'item', slot: 'template' }} label={t('guided.where.typesTable')} />
        </Suspense>
      )}
    </section>
  );
};

export default ItemTypes;
