/**
 * "Linked from": the records whose values point at one item or item set, one row per kind and
 * field with its exact count (`GET /api/romeka/links/{id}`). An items row opens the Items screen
 * filtered to those items; an item set or media row lists its records here, a few at a time,
 * through the same `res` search its count is the total of. Every read runs as the caller, so a
 * record the caller may not see is neither counted nor listed.
 *
 * Its own chunk: the item page and the set card load it with `lazy`.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { listMedia } from '../../api/media';
import { linksQuery, placesApi } from '../../api/places';
import type { LinkKind, LinkRow, LinkTarget } from '../../api/places';
import type { ApiPage } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import type { KeyOf } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';
import { useTermLabels } from '../../i18n/termText';
import { setPlaceHref } from '../item-sets/facts';
import { mediaHref } from '../media/annotations/link';
import { linksPlaceHref } from './browse';
import './linked-from.css';

const RECORDS_PER_PAGE = 5;

interface RecordLink {
  readonly id: number;
  readonly href: string;
  readonly title: string | null;
}

type CountKey = KeyOf<typeof resources>;

/** The count's words per kind, singular and plural. */
const COUNT_WORDS: Readonly<Record<LinkKind, readonly [CountKey, CountKey]>> = {
  items: ['setPlace.countOne', 'setPlace.countMany'],
  item_sets: ['linkedFrom.setsOne', 'linkedFrom.setsMany'],
  media: ['linkedFrom.mediaOne', 'linkedFrom.mediaMany'],
};

const loadRecords = async (
  kind: Exclude<LinkKind, 'items'>,
  target: LinkTarget,
  row: LinkRow,
  page: number,
): Promise<ApiPage<RecordLink>> => {
  const query = { ...linksQuery(target, row.propertyId), page, perPage: RECORDS_PER_PAGE, sortBy: 'id', sortOrder: 'asc' } as const;
  if (kind === 'item_sets') {
    const sets = await api.listItemSets(query);
    return { ...sets, items: sets.items.map((set) => ({ id: set.id, href: setPlaceHref(set.id), title: set.title })) };
  }
  const media = await listMedia(api, query);
  return { ...media, items: media.items.map((entry) => ({ id: entry.id, href: mediaHref(entry.id), title: entry.title })) };
};

/** The records of one item set or media row, a page at a time, each page added under the last. */
const LinkedRecords = ({
  kind,
  target,
  row,
}: {
  readonly kind: Exclude<LinkKind, 'items'>;
  readonly target: LinkTarget;
  readonly row: LinkRow;
}): ReactElement => {
  const { t } = useTranslator(common, resources);
  const [page, setPage] = useState(1);
  const [loaded, setLoaded] = useState<readonly ApiPage<RecordLink>[]>([]);
  const state = useAsync(() => loadRecords(kind, target, row, page), [page]);
  if (state.status === 'ready' && !loaded.includes(state.data)) {
    setLoaded([...loaded, state.data]);
  }
  const last = loaded.at(-1);
  const records = loaded.flatMap((entry) => entry.items);
  return (
    <div className="linked-records">
      {records.length === 0 ? null : (
        <ul>
          {records.map((record) => (
            <li key={record.id}>
              <a href={record.href}>{record.title ?? t('common.untitled')}</a>
            </li>
          ))}
        </ul>
      )}
      {state.status === 'loading' ? <p role="status">…</p> : null}
      {state.status === 'error' ? <ErrorNotice error={state.error} /> : null}
      {state.status === 'ready' && last !== undefined && last.page < last.lastPage ? (
        <button
          type="button"
          className="link-button"
          onClick={() => {
            setPage(last.page + 1);
          }}
        >
          {t('linkedFrom.more')}
        </button>
      ) : null}
    </div>
  );
};

/** A row's words, for example "Creator · 2 items"; the item page's delete states them too. */
export const useLinkRowWords = (): ((row: LinkRow) => string) => {
  const { t, number } = useTranslator(resources);
  const text = useTermLabels();
  return (row) => {
    const [one, many] = COUNT_WORDS[row.kind];
    const count = row.count === 1 ? t(one) : t(many, { count: number(row.count) });
    return t('linkedFrom.row', { field: text.label(row.term, row.label), count });
  };
};

const RowLine = ({ target, row }: { readonly target: LinkTarget; readonly row: LinkRow }): ReactElement => {
  const rowWords = useLinkRowWords();
  const [open, setOpen] = useState(false);
  const words = rowWords(row);
  if (row.kind === 'items') {
    return <a href={linksPlaceHref(target, row.propertyId)}>{words}</a>;
  }
  return (
    <>
      <button
        type="button"
        className="link-button"
        aria-expanded={open}
        onClick={() => {
          setOpen(!open);
        }}
      >
        {words}
      </button>
      {open ? <LinkedRecords kind={row.kind} target={target} row={row} /> : null}
    </>
  );
};

export interface LinkedFromProps {
  readonly target: LinkTarget;
}

/** The rows, or one line for loading, for nothing, or for a failed read; never more than its own box. */
export const LinkedFrom = ({ target }: LinkedFromProps): ReactElement => {
  const { t } = useTranslator(resources);
  const state = useAsync(() => placesApi.links(target), [target]);
  if (state.status === 'loading') {
    return (
      <p className="linked-from" role="status">
        {t('linkedFrom.loading')}
      </p>
    );
  }
  if (state.status === 'error') {
    return (
      <div className="linked-from">
        <ErrorNotice error={state.error} />
      </div>
    );
  }
  if (state.data.length === 0) {
    return <p className="linked-from">{t('linkedFrom.empty')}</p>;
  }
  return (
    <ul className="linked-from">
      {state.data.map((row) => (
        <li key={`${row.kind}:${String(row.propertyId)}`}>
          <RowLine target={target} row={row} />
        </li>
      ))}
    </ul>
  );
};

export default LinkedFrom;
