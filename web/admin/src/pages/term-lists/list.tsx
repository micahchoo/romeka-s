import type { ReactElement } from 'react';
import { api } from '../../api/client';
import { listTermLists } from '../../api/term-lists';
import type { ApiPage, ItemSet, TermList } from '../../api/types';
import { ownerNameOf, userNames } from '../../app/labels';
import { ListPage, mapPage } from '../../app/list';
import { useTranslator } from '../../i18n';
import { useTermListText } from './messages';
import type { TermListMessageKey } from './messages';

const PER_PAGE = 25;

interface TermListRow {
  readonly list: TermList;
  readonly ownerName: string | null;
  readonly itemSetTitle: string | null;
  /** Items carrying at least one value picked from the list. */
  readonly usage: number;
}

export const KIND_LABEL: Readonly<Record<TermList['source']['kind'], TermListMessageKey>> = {
  terms: 'termLists.kindTerms',
  uris: 'termLists.kindUris',
  items: 'termLists.kindItems',
  empty: 'termLists.kindEmpty',
};

/**
 * How many items use a list: the `dt` property query counts them. The empty
 * `property` means any property; the port drops a row that omits the key.
 */
const usage = async (list: TermList): Promise<number> =>
  (
    await api.listItems({
      perPage: 1,
      'property[0][property]': '',
      'property[0][type]': 'dt',
      'property[0][text]': list.dataType,
    })
  ).total;

const itemSetTitle = (sets: readonly ItemSet[], list: TermList): string | null => {
  if (list.source.kind !== 'items') {
    return null;
  }
  const id = list.source.itemSet.id;
  return sets.find((set) => set.id === id)?.title ?? `#${String(id)}`;
};

const load = async (page: number): Promise<ApiPage<TermListRow>> => {
  const [lists, users, sets] = await Promise.all([
    listTermLists(api, { page, perPage: PER_PAGE, sortBy: 'label' }),
    api.listUsers({ perPage: 100 }),
    api.listItemSets({ perPage: 200 }),
  ]);
  const counts = await Promise.all(lists.items.map(usage));
  const names = userNames(users.items);
  return mapPage(lists, (list) => ({
    list,
    ownerName: ownerNameOf(names, list.owner),
    itemSetTitle: itemSetTitle(sets.items, list),
    usage: counts[lists.items.indexOf(list)] ?? 0,
  }));
};

const entries = (row: TermListRow): string | number => {
  const source = row.list.source;
  switch (source.kind) {
    case 'terms':
      return source.terms.length;
    case 'uris':
      return source.uris.length;
    case 'items':
      return row.itemSetTitle ?? '';
    case 'empty':
      return 0;
  }
};

/** `CustomVocab\Controller\Admin\IndexController::browseAction`, plus usage. */
export const TermListList = (): ReactElement => {
  const { t } = useTranslator();
  const tl = useTermListText();
  const resource = tl('termLists.resource');
  return (
    <div>
      <p>{tl('termLists.intro')}</p>
      <p>
        <a href="#/term-lists/new">{tl('termLists.add')}</a>
      </p>
      <ListPage<TermListRow>
        resourceLabel={resource}
        emptyMessage={t('list.empty', { resource })}
        load={load}
        rowKey={(row) => String(row.list.id)}
        columns={[
          {
            header: t('column.label'),
            cell: (row) => <a href={`#/term-lists/${String(row.list.id)}`}>{row.list.label}</a>,
          },
          { header: tl('termLists.columnKind'), cell: (row) => tl(KIND_LABEL[row.list.source.kind]) },
          { header: tl('termLists.columnEntries'), cell: entries },
          { header: tl('termLists.columnUsage'), cell: (row) => row.usage, numeric: true },
          { header: t('column.owner'), cell: (row) => row.ownerName ?? t('common.noOwner') },
        ]}
      />
    </div>
  );
};
