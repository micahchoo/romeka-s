/**
 * The items in groups that share facts (`shapes`): each group says what its
 * items have, shows one of them, offers another, opens all of them in the
 * items list, and takes one change for all of them. Advanced's archive node
 * places it.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import { importApi } from '../../../api/import-routes';
import type { RecordView } from '../../../api/import-model';
import type { ShapeGroup } from '../../../api/import-sections';
import { ErrorNotice } from '../../../app/errors';
import { Pager } from '../../../app/list';
import { useDraft } from '../draft-context';
import { useListsText } from './messages';
import { listHref } from './address';
import { ChangeMany } from './change-many';
import { factsWords, foldersWords } from './facts-words';
import { fileEntries, gather, itemEntry } from './gather';
import { fileName, recordName, recordTitle } from './record-words';
import { shapeKey } from './shape-filter';
import { usePreview } from './use-preview';

export const GROUPS_PER_PAGE = 20;
const FILES_SHOWN = 4;

const Example = ({ record }: { readonly record: RecordView }): ReactElement => {
  const { t, number } = useListsText();
  const title = recordTitle(record);
  const more = record.files.length - FILES_SHOWN;
  return (
    <div className="import-example">
      <p>
        <b>{recordName(record)}</b>
        {title === null || title === recordName(record) ? null : <> · {title}</>}
      </p>
      <ul>
        {record.files.length === 0 ? <li className="muted">{t('lists.groups.noFile')}</li> : null}
        {record.files.slice(0, FILES_SHOWN).map((file) => (
          <li key={file.path}>{fileName(file)}</li>
        ))}
        {more > 0 ? <li className="muted">{more === 1 ? t('lists.groups.moreOne') : t('lists.groups.more', { count: number(more) })}</li> : null}
      </ul>
    </div>
  );
};

const Group = ({ group, shown, onAnother }: { readonly group: ShapeGroup; readonly shown: number; readonly onAnother: () => void }): ReactElement => {
  const { t, number } = useListsText();
  const draft = useDraft();
  const sample = group.samples[shown % Math.max(1, group.samples.length)];
  const items = draft.place.mode === 'simple' ? ({ mode: 'simple', screen: 'all-items' } as const) : ({ mode: 'advanced', screen: 'items' } as const);
  const load = (page: number, perPage: number) =>
    importApi.preview(draft.id, ['records'], { records: { kind: 'item', shape: group.facts, page, perPage } }).then((preview) => preview.records);
  const several = group.facts.files !== '0' && group.facts.files !== '1';
  return (
    <article className="import-group">
      <p>
        <b>{group.count === 1 ? t('lists.groups.countOne') : t('lists.groups.count', { count: number(group.count) })}</b>{' '}
        {factsWords(group.facts, draft.plan, t)}. <span className="muted">{foldersWords(group.facts, t)}</span>
      </p>
      {group.odd ? <p className="import-pill">{t('lists.groups.odd')}</p> : null}
      {sample === undefined ? null : <Example record={sample} />}
      <p className="import-row-actions">
        {group.count > 1 ? (
          <button type="button" onClick={onAnother}>
            {t('lists.groups.another')}
          </button>
        ) : null}
        <a href={listHref(draft.id, items, { group: shapeKey(group.facts) })}>{t('lists.groups.all', { count: number(group.count) })}</a>
      </p>
      <ChangeMany
        count={group.count}
        noun="items"
        choices={[
          ...(several ? [{ kind: 'button' as const, label: t('lists.groups.split'), change: { kind: 'move', to: 'own' } as const }] : []),
          { kind: 'button', label: t('lists.many.leave'), change: { kind: 'leave' } },
        ]}
        gather={(onRead, signal) => gather(load, several ? fileEntries : itemEntry, recordName, onRead, signal)}
      />
    </article>
  );
};

export const Groups = (): ReactElement => {
  const { t, number } = useListsText();
  const [page, setPage] = useState(1);
  const [seed, setSeed] = useState(0);
  const [shown, setShown] = useState<Readonly<Record<string, number>>>({});
  const read = usePreview(['shapes'], { shapes: { page, perPage: GROUPS_PER_PAGE }, seed });
  if (read.status === 'loading') {
    return <p role="status" className="import-slow">{t('paged.loading', { page })}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const { shapes } = read.data;
  const lastPage = Math.max(1, Math.ceil(shapes.groups.total / shapes.groups.perPage));
  return (
    <section className="import-groups" aria-busy={read.busy} aria-label={t('lists.groups.label')}>
      <p>{t('lists.groups.lede', { items: number(shapes.records), groups: number(shapes.groups.total) })}</p>
      {shapes.groups.items.map((group) => {
        const key = shapeKey(group.facts);
        const at = shown[key] ?? 0;
        return (
          <Group
            key={key}
            group={group}
            shown={at}
            onAnother={() => {
              // Three samples come with each group; past them, a new seed draws three more.
              if ((at + 1) % Math.max(1, group.samples.length) === 0) {
                setSeed(seed + 1);
              }
              setShown({ ...shown, [key]: at + 1 });
            }}
          />
        );
      })}
      {lastPage > 1 ? <Pager page={page} lastPage={lastPage} total={shapes.groups.total} label={t('lists.groups.resource')} onPage={setPage} /> : null}
    </section>
  );
};

export default Groups;
