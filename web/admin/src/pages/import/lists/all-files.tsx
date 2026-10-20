/**
 * Every file of the import (Simple and Advanced "Every file"): in an item or
 * not, why not, by folder and by name, 50 to a page. One file can be made an
 * item of its own or left out; a filter's files can be changed all at once.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import { importApi } from '../../../api/import-routes';
import type { EntryView } from '../../../api/import-model';
import type { Change, FileRef, MoveTo } from '../../../api/import-plan';
import type { EntriesQuery, Unused } from '../../../api/import-queries';
import { UNUSED_REASONS } from '../../../api/import-queries';
import type { Paged } from '../../../api/import-sections';
import type { Column } from '../../../app/list';
import { useHash } from '../../../app/router';
import { useDraft } from '../draft-context';
import { Filters } from '../filters';
import type { FilterField, FilterValue } from '../filters';
import { useImportText } from '../messages';
import type { ImportMessageKey } from '../messages';
import { PagedList } from '../paged-list';
import { filterFromHash } from './address';
import { ChangeMany } from './change-many';
import { ChangesMade } from './changes-made';
import { gather } from './gather';
import { fileName, folderOf, leftWords } from './record-words';

export const FILE_FILTERS = ['text', 'used', 'reason', 'folder'] as const;
export type FileFilter = (typeof FILE_FILTERS)[number];

const REASON_WORDS: Readonly<Record<Unused, ImportMessageKey>> = {
  unmatched: 'lists.files.why.unmatched',
  decision: 'lists.files.why.decision',
  flag: 'lists.files.why.flag',
  parent: 'lists.files.why.parent',
  noFiles: 'lists.files.why.noFiles',
  noRecord: 'lists.files.why.noRecord',
};

export const entriesQuery = (filter: FilterValue<FileFilter>): EntriesQuery => {
  const reason = UNUSED_REASONS.find((one) => one === filter.reason);
  return {
    // A folder filter, even the empty one, answers files only: list rows have no folder (`preview/query.rs`).
    folder: filter.folder ?? '',
    ...(filter.text === undefined ? {} : { text: filter.text }),
    ...(filter.used === undefined ? {} : { used: filter.used === 'yes' }),
    ...(reason === undefined ? {} : { reason }),
  };
};

const fileOf = (entry: EntryView): FileRef | null => ('file' in entry.entry ? entry.entry.file : null);

const fileOnly = (entry: EntryView): readonly { readonly file: FileRef }[] => {
  const file = fileOf(entry);
  return file === null ? [] : [{ file }];
};

const move = (to: MoveTo): Change => ({ kind: 'move', to });

export const AllFiles = (): ReactElement => {
  const { t, number } = useImportText();
  const draft = useDraft();
  const hash = useHash();
  const [filter, setFilter] = useState<FilterValue<FileFilter>>(() => filterFromHash(hash, FILE_FILTERS));
  const [total, setTotal] = useState<number | null>(null);
  const query = entriesQuery(filter);
  const load = (page: number, perPage: number): Promise<Paged<EntryView>> =>
    importApi.preview(draft.id, ['entries'], { entries: { ...query, page, perPage } }).then((preview) => preview.entries);
  const narrowed = Object.keys(filter).length > 0;

  const fields: readonly FilterField<FileFilter>[] = [
    { key: 'text', kind: 'text', label: t('lists.files.text') },
    {
      key: 'used',
      kind: 'choice',
      label: t('lists.files.used'),
      any: t('lists.any'),
      options: [
        { value: 'yes', label: t('lists.files.onlyUsed') },
        { value: 'no', label: t('lists.files.onlyUnused') },
      ],
    },
    {
      key: 'reason',
      kind: 'choice',
      label: t('lists.files.reason'),
      any: t('lists.any'),
      options: UNUSED_REASONS.map((reason) => ({ value: reason, label: t(REASON_WORDS[reason]) })),
    },
    { key: 'folder', kind: 'text', label: t('lists.files.folder') },
  ];

  const changeOne = (file: FileRef, to: MoveTo): void => {
    draft.change((plan) => ({ ...plan, decisions: [...plan.decisions, { kind: 'move', file: { file }, to }] }));
  };

  const columns: readonly Column<EntryView>[] = [
    { header: t('lists.files.colName'), cell: (entry) => { const file = fileOf(entry); return file === null ? '' : fileName(file); } },
    {
      header: t('lists.files.colFolder'),
      cell: (entry) => {
        const file = fileOf(entry);
        const folder = file === null ? '' : folderOf(file);
        return folder === '' ? <span className="muted">{t('lists.files.top')}</span> : folder;
      },
    },
    {
      header: t('lists.files.colItem'),
      cell: (entry) => {
        if (entry.left !== null) {
          return <span className="muted">{leftWords(entry.left, t)}</span>;
        }
        if (entry.unmatched) {
          return <span className="muted">{t('lists.files.why.unmatched')}</span>;
        }
        return entry.record === null ? <span className="muted">{t('lists.files.why.noRecord')}</span> : t('lists.files.inItem');
      },
    },
    {
      header: t('lists.files.colChange'),
      cell: (entry) => {
        const file = fileOf(entry);
        if (file === null) {
          return null;
        }
        return (
          <select
            aria-label={t('lists.files.changeOne', { file: fileName(file) })}
            value=""
            disabled={!draft.editable}
            onChange={(event) => {
              if (event.target.value === 'own' || event.target.value === 'leave') {
                changeOne(file, event.target.value);
              }
            }}
          >
            <option value="">{t('lists.files.change')}</option>
            <option value="own">{t('lists.files.own')}</option>
            <option value="leave">{t('lists.files.leave')}</option>
          </select>
        );
      },
    },
  ];

  return (
    <div className="import-files">
      <p className="lede">{t('lists.files.lede')}</p>
      <Filters
        label={t('lists.files.filters')}
        fields={fields}
        value={filter}
        onChange={(value) => {
          setTotal(null);
          setFilter(value);
        }}
      />
      {total === null ? null : (
        <p className="import-count" aria-live="polite">
          <b>{total === 1 ? t('lists.files.countOne') : t('lists.files.count', { count: number(total) })}</b>
          {narrowed ? ` ${t('lists.withFilters')}` : null}
        </p>
      )}
      {narrowed && total !== null && total > 0 ? (
        <ChangeMany
          key={JSON.stringify(filter)}
          count={total}
          noun="files"
          choices={[
            { kind: 'button', label: t('lists.files.manyLeave'), change: move('leave') },
            { kind: 'button', label: t('lists.files.manyOwn'), change: move('own') },
          ]}
          gather={(onRead, signal) =>
            gather(load, fileOnly, (entry) => { const file = fileOf(entry); return file === null ? '' : fileName(file); }, onRead, signal)
          }
        />
      ) : null}
      <PagedList
        label={t('lists.files.table')}
        resource={t('lists.files.resource')}
        load={load}
        deps={[draft.detail.revision, JSON.stringify(query)]}
        columns={columns}
        rowKey={(entry) => { const file = fileOf(entry); return file === null ? '' : `${file.source}/${file.path}`; }}
        empty={<p>{narrowed ? t('lists.files.noneFiltered') : t('lists.files.none')}</p>}
        onPage={(page) => {
          setTotal(page.total);
        }}
      />
      <ChangesMade />
    </div>
  );
};

export default AllFiles;
