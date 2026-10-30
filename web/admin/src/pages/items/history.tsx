/**
 * The item page's History: each recorded change, newest first, as the server read it for this
 * caller (`GET /api/romeka/items/{id}/history`). A line says when, who and what made it ("Oct 22,
 * 2026 · Admin · Bulk change 12"), then each changed field before and after ("Date: 1950 →
 * c. 1950"). "Put this value back" is an ordinary edit of the item through `PUT /api/items/{id}`,
 * so it is itself recorded and appears at the top.
 *
 * Its own chunk: the item page loads it with `lazy` when History opens.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { loadItemHistory } from '../../api/item-history';
import type { HistoryChange, HistoryEntry, HistorySet, HistorySource, HistoryValue } from '../../api/item-history';
import type { Item, ItemId, ItemWrite } from '../../api/types';
import { resourceDraftFrom, toValueWrites } from '../../app/draft';
import { ErrorNotice } from '../../app/errors';
import { formatDateTime } from '../../app/format';
import { toError, useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import type { MessageParams } from '../../i18n/catalogue';
import { en, itemHistory } from '../../i18n/namespaces/item-history';
import { useTermLabels } from '../../i18n/termText';
import './history.css';
import { plainName } from '../../app/plain-terms';
import { common } from '../../i18n/namespaces/common';

const ENTRIES_PER_PAGE = 20;

type Translate = (key: keyof typeof en, params?: MessageParams) => string;

/** The item as an edit would write it now, with one field set back to its `before`. */
export const putBackWrite = (item: Item, change: HistoryChange): ItemWrite => {
  const draft = resourceDraftFrom(item);
  const write: ItemWrite = {
    isPublic: draft.isPublic,
    owner: draft.owner,
    resourceClass: draft.resourceClass,
    resourceTemplate: draft.resourceTemplate,
    thumbnail: draft.thumbnail,
    values: toValueWrites(draft.values),
    itemSets: item.itemSets.map((ref) => ref.id),
    primaryMedia: item.primaryMedia?.id ?? null,
    sites: item.sites.map((ref) => ref.id),
  };
  switch (change.field) {
    case 'values': {
      const at = write.values.findIndex((value) => value.propertyId === change.propertyId);
      const others = write.values.filter((value) => value.propertyId !== change.propertyId);
      const back = change.before.map((value) => value.write);
      const place = at === -1 ? others.length : at;
      return { ...write, values: [...others.slice(0, place), ...back, ...others.slice(place)] };
    }
    case 'visibility':
      return { ...write, isPublic: change.before ?? write.isPublic };
    case 'class':
      return { ...write, resourceClass: change.before?.id ?? null };
    case 'template':
      return { ...write, resourceTemplate: change.before?.id ?? null };
    case 'item_sets':
      return { ...write, itemSets: change.before.map((set) => set.id) };
    case 'sites':
      return { ...write, sites: change.before.map((site) => site.id) };
  }
};

/**
 * Whether the change has a state to go back to. A field that had no value is emptied in the form.
 * A set in the trash or deleted for good cannot be joined again, so its change has none.
 */
const canPutBack = (change: HistoryChange): boolean => {
  switch (change.field) {
    case 'values':
      return change.before.length > 0;
    case 'visibility':
      return change.before !== null;
    case 'item_sets':
      return change.before.every((set) => set.gone === null);
    default:
      return true;
  }
};

const sourceWords = (source: HistorySource, t: Translate): string | null => {
  switch (source.kind) {
    case 'edit':
      return null;
    case 'bulk':
      return t('itemHistory.source.bulk', { id: String(source.id) });
    case 'import':
      return t('itemHistory.source.import', { id: String(source.id) });
    case 'bulk_undo':
      return t('itemHistory.source.bulkUndo', { id: String(source.id) });
    case 'import_undo':
      return t('itemHistory.source.importUndo', { id: String(source.id) });
  }
};

const joined = (words: readonly string[], t: Translate): string =>
  words.length === 0 ? t('itemHistory.none') : words.join(', ');

const valueWords = (values: readonly HistoryValue[], t: Translate): string =>
  joined(
    values.map((value) =>
      value.write.isPublic ? value.display : t('itemHistory.privateValue', { value: value.display }),
    ),
    t,
  );

/** The field's name and its two sides, in words. */
const useChangeWords = (): ((change: HistoryChange) => readonly [string, string, string]) => {
  const { t } = useTranslator(itemHistory, common);
  const terms = useTermLabels();
  const named = (side: { readonly id: number; readonly label: string | null } | null): string =>
    side === null ? t('itemHistory.none') : (side.label ?? t('common.untitledNumber', { id: String(side.id) }));
  const titled = (sides: readonly { readonly id: number; readonly title: string | null }[]): string =>
    joined(sides.map((side) => side.title ?? t('common.untitledNumber', { id: String(side.id) })), t);
  const setWords = (sets: readonly HistorySet[]): string =>
    joined(
      sets.map((set) => {
        const id = String(set.id);
        if (set.gone === 'deleted') {
          return t('itemHistory.deletedSet', { id });
        }
        const name = set.title ?? t('common.untitledNumber', { id });
        return set.gone === 'trash' ? t('itemHistory.inTrash', { name }) : name;
      }),
      t,
    );
  return (change) => {
    switch (change.field) {
      case 'values':
        return [plainName(change, terms), valueWords(change.before, t), valueWords(change.after, t)];
      case 'visibility': {
        const word = (open: boolean): string => (open ? t('itemHistory.public') : t('itemHistory.private'));
        return [
          t('itemHistory.field.visibility'),
          change.before === null ? t('itemHistory.none') : word(change.before),
          word(change.after),
        ];
      }
      case 'class':
        return [t('itemHistory.field.class'), named(change.before), named(change.after)];
      case 'template':
        return [t('itemHistory.field.template'), named(change.before), named(change.after)];
      case 'item_sets':
        return [t('itemHistory.field.itemSets'), setWords(change.before), setWords(change.after)];
      case 'sites':
        return [t('itemHistory.field.sites'), titled(change.before), titled(change.after)];
    }
  };
};

type PutBackState =
  | { readonly status: 'idle' }
  | { readonly status: 'saving' }
  | { readonly status: 'done'; readonly changed: boolean }
  | { readonly status: 'error'; readonly error: Error };

const ChangeLine = ({
  change,
  busy,
  onPutBack,
}: {
  readonly change: HistoryChange;
  readonly busy: boolean;
  readonly onPutBack: (change: HistoryChange) => void;
}): ReactElement => {
  const { t } = useTranslator(itemHistory);
  const [field, before, after] = useChangeWords()(change);
  return (
    <li>
      <span>{t('itemHistory.change', { field, before, after })}</span>
      {canPutBack(change) ? (
        <button
          type="button"
          className="link-button"
          disabled={busy}
          onClick={() => {
            onPutBack(change);
          }}
        >
          {t('itemHistory.putBack')}
        </button>
      ) : null}
    </li>
  );
};

const EntryLine = ({
  entry,
  busy,
  onPutBack,
}: {
  readonly entry: HistoryEntry;
  readonly busy: boolean;
  readonly onPutBack: (change: HistoryChange) => void;
}): ReactElement => {
  const { t, locale } = useTranslator(itemHistory);
  const date = formatDateTime(entry.created, locale);
  const who = entry.user?.name ?? t('itemHistory.deletedUser');
  const source = sourceWords(entry.source, t);
  return (
    <li>
      <p className="item-history-when">
        {source === null ? t('itemHistory.entry', { date, who }) : t('itemHistory.entrySource', { date, who, source })}
      </p>
      <ul>
        {entry.changes.map((change, at) => (
          <ChangeLine key={`${change.field}:${String(at)}`} change={change} busy={busy} onPutBack={onPutBack} />
        ))}
      </ul>
    </li>
  );
};

export interface ItemHistoryProps {
  readonly item: ItemId;
  /** The item as the put-back saved it, for the page to show. */
  readonly onSaved: (item: Item) => void;
}

export const ItemHistory = ({ item, onSaved }: ItemHistoryProps): ReactElement => {
  const { t } = useTranslator(itemHistory);
  const [reads, setReads] = useState(0);
  const [shown, setShown] = useState(ENTRIES_PER_PAGE);
  const [putBack, setPutBack] = useState<PutBackState>({ status: 'idle' });
  const state = useAsync(() => loadItemHistory(item), [item, reads]);

  const put = (newest: number | null) => (change: HistoryChange): void => {
    setPutBack({ status: 'saving' });
    void (async () => {
      try {
        const saved = await api.updateItem(item, putBackWrite(await api.getItem(item), change));
        const after = await loadItemHistory(item);
        onSaved(saved);
        setPutBack({ status: 'done', changed: (after[0]?.id ?? null) !== newest });
        setReads((count) => count + 1);
      } catch (error: unknown) {
        setPutBack({ status: 'error', error: toError(error) });
      }
    })();
  };

  if (state.status === 'loading') {
    return (
      <p className="item-history" role="status">
        {t('itemHistory.loading')}
      </p>
    );
  }
  if (state.status === 'error') {
    return (
      <div className="item-history">
        <ErrorNotice error={state.error} />
      </div>
    );
  }
  const entries = state.data;
  return (
    <div className="item-history">
      {putBack.status === 'saving' ? <p role="status">{t('itemHistory.puttingBack')}</p> : null}
      {putBack.status === 'done' ? (
        <p role="status">{putBack.changed ? t('itemHistory.putBackDone') : t('itemHistory.putBackSame')}</p>
      ) : null}
      {putBack.status === 'error' ? <ErrorNotice error={putBack.error} /> : null}
      {entries.length === 0 ? (
        <p>{t('itemHistory.empty')}</p>
      ) : (
        <ul className="item-history-entries">
          {entries.slice(0, shown).map((entry) => (
            <EntryLine
              key={entry.id}
              entry={entry}
              busy={putBack.status === 'saving'}
              onPutBack={put(entries[0]?.id ?? null)}
            />
          ))}
        </ul>
      )}
      {entries.length > shown ? (
        <button
          type="button"
          className="link-button"
          onClick={() => {
            setShown(shown + ENTRIES_PER_PAGE);
          }}
        >
          {t('itemHistory.older')}
        </button>
      ) : null}
    </div>
  );
};

export default ItemHistory;
