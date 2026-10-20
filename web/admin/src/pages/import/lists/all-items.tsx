/**
 * Every item the import would add (Simple "Every item", Advanced "Items"):
 * filtered by the facts the server filters on, 50 to a page, and changed all
 * at once for whatever the filter selects (`docs/import-v2.md`, "Filters on
 * records" and "Changes to many records").
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import { importApi } from '../../../api/import-routes';
import type { RecordView } from '../../../api/import-model';
import type { Change, DecisionValue, FlagKind, ImportPlan } from '../../../api/import-plan';
import { FLAG_KINDS } from '../../../api/import-plan';
import type { RecordsQuery, SetFilter, TemplateFilter } from '../../../api/import-queries';
import { BUCKETS } from '../../../api/import-sections';
import type { Check, Paged, ShapeGroup } from '../../../api/import-sections';
import type { Column } from '../../../app/list';
import { useAsync } from '../../../app/useAsync';
import { useHash } from '../../../app/router';
import { useDraft } from '../draft-context';
import { Filters } from '../filters';
import type { FilterField, FilterValue } from '../filters';
import { useImportText } from '../messages';
import type { ImportTranslator } from '../messages';
import { PagedList } from '../paged-list';
import { filterFromHash } from './address';
import { loadCatalog } from './catalog';
import type { Catalog } from './catalog';
import { ChangeMany } from './change-many';
import type { ChangeChoice } from './change-many';
import { ChangesMade } from './changes-made';
import { BUCKET_WORDS, factsWords } from './facts-words';
import { gather, itemEntry } from './gather';
import { leftWords, recordName, recordTitle } from './record-words';
import { shapeFromKey, shapeKey } from './shape-filter';
import { usePreview } from './use-preview';

export const ITEM_FILTERS = ['text', 'group', 'problem', 'folder', 'set', 'type', 'files', 'left'] as const;
export type ItemFilter = (typeof ITEM_FILTERS)[number];

const isFlag = (text: string | undefined): text is FlagKind => FLAG_KINDS.some((kind) => kind === text);

const setFilter = (text: string | undefined): SetFilter | undefined => {
  const [kind, id] = (text ?? '').split(':');
  const n = Number(id);
  if (!Number.isSafeInteger(n) || id === undefined || id === '') {
    return undefined;
  }
  return kind === 'existing' ? { existing: n } : kind === 'new' ? { new: n } : undefined;
};

const templateFilter = (text: string | undefined): TemplateFilter | undefined => {
  if (text === 'none') {
    return 'none';
  }
  const ref = setFilter(text);
  if (ref === undefined || 'record' in ref) {
    return undefined;
  }
  return 'existing' in ref ? { existing: ref.existing } : { new: ref.new };
};

/** What the filter row asks the server; a part the server would refuse is left out. */
export const recordsQuery = (filter: FilterValue<ItemFilter>): RecordsQuery => {
  const shape = filter.group === undefined ? null : shapeFromKey(filter.group);
  const set = setFilter(filter.set);
  const template = templateFilter(filter.type);
  const files = BUCKETS.find((bucket) => bucket === filter.files);
  return {
    kind: 'item',
    ...(filter.text === undefined ? {} : { text: filter.text }),
    ...(filter.folder === undefined ? {} : { folder: filter.folder }),
    ...(shape === null ? {} : { shape }),
    ...(isFlag(filter.problem) ? { flag: filter.problem } : {}),
    ...(set === undefined ? {} : { set }),
    ...(template === undefined ? {} : { template }),
    ...(files === undefined ? {} : { files }),
    ...(filter.left === undefined ? {} : { left: filter.left === 'out' }),
  };
};

const literal = (text: string): DecisionValue => ({ type: 'literal', text, lang: null, label: null });

const setChange = (text: string): Change => ({ kind: 'set', target: 'item', slot: 'sets', values: [literal(text)] });
const typeChange = (text: string): Change => ({ kind: 'set', target: 'item', slot: 'template', values: [literal(text)] });

/** The changes the bar offers: a set or a type is sent by id, a new one by its title. */
const choicesOf = (catalog: Catalog | null, check: Check | null, plan: ImportPlan, t: ImportTranslator['t']): readonly ChangeChoice[] => {
  const sets = [
    ...(catalog?.itemSets ?? []).map((set) => ({ name: set.title ?? `#${String(set.id)}`, text: String(set.id) })),
    ...(check?.newSets ?? []).map((set) => ({ name: set.title, text: set.title })),
  ];
  const types = [
    ...(catalog?.templates ?? []).map((template) => ({ name: template.label, text: String(template.id) })),
    ...plan.newTypes.map((type) => ({ name: type.label, text: type.label })),
  ];
  return [
    {
      kind: 'select',
      label: t('lists.many.setPick'),
      options: sets.slice(0, 200).map((set) => ({ label: t('lists.many.toSet', { set: set.name }), change: setChange(set.text) })),
    },
    {
      kind: 'select',
      label: t('lists.many.typePick'),
      options: types.slice(0, 200).map((type) => ({ label: t('lists.many.toType', { type: type.name }), change: typeChange(type.text) })),
    },
    { kind: 'button', label: t('lists.many.leave'), change: { kind: 'leave' } },
  ];
};

const fieldsOf = (
  groups: Paged<ShapeGroup> | null,
  kinds: readonly FlagKind[],
  catalog: Catalog | null,
  check: Check | null,
  plan: ImportPlan,
  text: ImportTranslator,
): readonly FilterField<ItemFilter>[] => {
  const { t, number } = text;
  return [
    { key: 'text', kind: 'text', label: t('lists.items.text') },
    {
      key: 'group',
      kind: 'choice',
      label: t('lists.items.group'),
      any: t('lists.any'),
      options: (groups?.items ?? []).map((group) => ({
        value: shapeKey(group.facts),
        label: `${number(group.count)}: ${factsWords(group.facts, plan, t)}`,
      })),
    },
    {
      key: 'problem',
      kind: 'choice',
      label: t('lists.items.problem'),
      any: t('lists.any'),
      options: kinds.map((kind) => ({ value: kind, label: t(`problem.${kind}`) })),
    },
    { key: 'folder', kind: 'text', label: t('lists.items.folder') },
    {
      key: 'set',
      kind: 'choice',
      label: t('lists.items.set'),
      any: t('lists.any'),
      options: [
        ...(catalog?.itemSets ?? []).slice(0, 200).map((set) => ({ value: `existing:${String(set.id)}`, label: set.title ?? `#${String(set.id)}` })),
        ...(check?.newSets ?? []).map((set, index) => ({ value: `new:${String(index)}`, label: t('lists.items.newSet', { set: set.title }) })),
      ],
    },
    {
      key: 'type',
      kind: 'choice',
      label: t('lists.items.type'),
      any: t('lists.any'),
      options: [
        { value: 'none', label: t('lists.none') },
        ...(catalog?.templates ?? []).slice(0, 200).map((template) => ({ value: `existing:${String(template.id)}`, label: template.label })),
        ...plan.newTypes.map((type, index) => ({ value: `new:${String(index)}`, label: t('lists.items.newType', { type: type.label }) })),
      ],
    },
    {
      key: 'files',
      kind: 'choice',
      label: t('lists.items.files'),
      any: t('lists.any'),
      options: BUCKETS.map((bucket) => ({ value: bucket, label: t(BUCKET_WORDS[bucket]) })),
    },
    {
      key: 'left',
      kind: 'choice',
      label: t('lists.items.left'),
      any: t('lists.any'),
      options: [
        { value: 'in', label: t('lists.items.onlyIn') },
        { value: 'out', label: t('lists.items.onlyOut') },
      ],
    },
  ];
};

const typeName = (record: RecordView, catalog: Catalog | null, plan: ImportPlan, t: ImportTranslator['t']): string => {
  const template = record.resolved.template;
  if (template === null) {
    return t('lists.none');
  }
  if ('new' in template) {
    return plan.newTypes[template.new]?.label ?? t('lists.none');
  }
  return catalog?.templates.find((one) => one.id === template.existing)?.label ?? `#${String(template.existing)}`;
};

export const AllItems = (): ReactElement => {
  const text = useImportText();
  const { t, number } = text;
  const draft = useDraft();
  const hash = useHash();
  const [filter, setFilter] = useState<FilterValue<ItemFilter>>(() => filterFromHash(hash, ITEM_FILTERS));
  const [total, setTotal] = useState<number | null>(null);
  const facts = usePreview(['shapes', 'flags', 'check'], {});
  const catalog = useAsync(loadCatalog, []);
  const ready = facts.status === 'ready' ? facts.data : null;
  const known = catalog.status === 'ready' ? catalog.data : null;
  const kinds = [...new Set((ready?.flags ?? []).filter((flag) => flag.count > 0).map((flag) => flag.kind))];
  const query = recordsQuery(filter);
  const load = (page: number, perPage: number): Promise<Paged<RecordView>> =>
    importApi.preview(draft.id, ['records'], { records: { ...query, page, perPage } }).then((preview) => preview.records);
  const narrowed = Object.keys(filter).length > 0;

  const columns: readonly Column<RecordView>[] = [
    { header: t('lists.items.colId'), cell: recordName },
    { header: t('lists.items.colTitle'), cell: (record) => recordTitle(record) ?? <span className="muted">{t('lists.items.noTitle')}</span> },
    {
      header: t('lists.items.colSet'),
      cell: (record) => (record.slots.sets ?? []).map((value) => value.text).join(', ') || <span className="muted">{t('lists.none')}</span>,
    },
    { header: t('lists.items.colType'), cell: (record) => typeName(record, known, draft.plan, t) },
    { header: t('lists.items.colFiles'), numeric: true, cell: (record) => number(record.files.length) },
    {
      header: t('lists.items.colState'),
      cell: (record) => {
        if (typeof record.fate === 'object' && 'left' in record.fate) {
          return <span className="muted">{leftWords(record.fate.left, t)}</span>;
        }
        const flag = record.flags[0];
        if (flag !== undefined && filter.problem !== flag.kind) {
          return (
            <button
              type="button"
              className="link-button"
              onClick={() => {
                setFilter({ problem: flag.kind });
              }}
            >
              {t('lists.items.look', { problem: t(`problem.${flag.kind}`) })}
            </button>
          );
        }
        return record.fate === 'create' ? t('lists.fate.create') : t('lists.fate.act', { id: String(record.fate.act) });
      },
    },
  ];

  return (
    <div className="import-items">
      <p className="lede">{t('lists.items.lede')}</p>
      <Filters
        label={t('lists.items.filters')}
        fields={fieldsOf(ready?.shapes.groups ?? null, kinds, known, ready?.check ?? null, draft.plan, text)}
        value={filter}
        onChange={(value) => {
          setTotal(null);
          setFilter(value);
        }}
      />
      {total === null ? null : (
        <p className="import-count" aria-live="polite">
          <b>{total === 1 ? t('lists.items.countOne') : t('lists.items.count', { count: number(total) })}</b>
          {narrowed ? ` ${t('lists.withFilters')}` : null}
        </p>
      )}
      {narrowed && total !== null && total > 0 ? (
        <ChangeMany
          key={JSON.stringify(filter)}
          count={total}
          noun="items"
          choices={choicesOf(known, ready?.check ?? null, draft.plan, t)}
          gather={(onRead, signal) => gather(load, itemEntry, recordName, onRead, signal)}
        />
      ) : null}
      <PagedList
        label={t('lists.items.table')}
        resource={t('lists.items.resource')}
        load={load}
        deps={[draft.detail.revision, JSON.stringify(query)]}
        columns={columns}
        rowKey={(record) => String(record.id)}
        empty={<p>{narrowed ? t('lists.items.noneFiltered') : t('lists.items.none')}</p>}
        onPage={(page) => {
          setTotal(page.total);
        }}
      />
      <ChangesMade />
    </div>
  );
};

export default AllItems;
