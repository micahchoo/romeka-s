/**
 * The item picker's columns and tray: the sources, one result row, the peek of
 * the focused item and the tray of picks. `ItemPicker` owns the state and the
 * focus; these render it.
 */
import type { KeyboardEvent, ReactElement, Ref } from 'react';
import { useState } from 'react';
import { api } from '../../../../api/client';
import type { Item, ItemId, ItemSet, ItemSetId, Media } from '../../../../api/types';
import { mediaName, previewImageUrl } from '../../../../app/media';
import { useAsync } from '../../../../app/useAsync';
import { useTranslator } from '../../../../i18n';
import { common } from '../../../../i18n/namespaces/common';
import { sites } from '../../../../i18n/namespaces/sites';
import type { Source } from './itemPickerModel';
import { keyValues, moved, valueRows } from './itemPickerModel';

const PEEK_MEDIA = 8;

const Thumb = ({ url, className }: { readonly url: string | null | undefined; readonly className: string }): ReactElement =>
  url === null || url === undefined ? (
    <span className={className} aria-hidden="true" />
  ) : (
    <img className={className} src={url} alt="" />
  );

export interface SourcesProps {
  readonly sets: readonly ItemSet[];
  readonly counts: ReadonlyMap<ItemSetId | 'all', number>;
  readonly current: Source;
  /** `null` when the caller hides "Pick the whole set". */
  readonly pickedSets: readonly ItemSetId[] | null;
  readonly onSource: (source: Source) => void;
  readonly onPickSet: (set: ItemSetId) => void;
  /** Where each source's button registers itself, so Left from the results can reach it. */
  readonly sourceRef: (key: ItemSetId | 'all') => Ref<HTMLButtonElement>;
  readonly onRight: () => void;
}

/** The first column: every item, then each item set with its count. */
export const Sources = ({
  sets,
  counts,
  current,
  pickedSets,
  onSource,
  onPickSet,
  sourceRef,
  onRight,
}: SourcesProps): ReactElement => {
  const { t, number } = useTranslator(common, sites);
  const entries: readonly { readonly key: ItemSetId | 'all'; readonly source: Source; readonly title: string }[] = [
    { key: 'all', source: { kind: 'all' }, title: t('itemPicker.allItems') },
    ...sets.map((set) => ({
      key: set.id,
      source: { kind: 'set', id: set.id } as const,
      title: set.title ?? t('common.untitled'),
    })),
  ];
  const isCurrent = (source: Source): boolean =>
    source.kind === current.kind && (source.kind === 'all' || (current.kind === 'set' && current.id === source.id));
  const onKey = (event: KeyboardEvent<HTMLButtonElement>, at: number): void => {
    const step = event.key === 'ArrowDown' ? 1 : event.key === 'ArrowUp' ? -1 : 0;
    if (step !== 0) {
      event.preventDefault();
      const next = entries[Math.min(Math.max(at + step, 0), entries.length - 1)];
      if (next !== undefined) {
        onSource(next.source);
      }
    } else if (event.key === 'ArrowRight') {
      event.preventDefault();
      onRight();
    }
  };
  return (
    <ul className="item-picker-sources" aria-label={t('itemPicker.sources')}>
      {entries.map((entry, at) => {
        const count = counts.get(entry.key);
        const on = isCurrent(entry.source);
        return (
          <li key={String(entry.key)} className={on ? 'is-current' : undefined}>
            <button
              type="button"
              ref={sourceRef(entry.key)}
              aria-pressed={on}
              tabIndex={on ? 0 : -1}
              onClick={() => {
                onSource(entry.source);
              }}
              onKeyDown={(event) => {
                onKey(event, at);
              }}
            >
              <span className="item-picker-source-title">{entry.title}</span>
              {count === undefined ? null : (
                <span className="item-picker-count">{t('itemPicker.setCount', { count: number(count) })}</span>
              )}
            </button>
            {pickedSets === null || entry.key === 'all' ? null : (
              <button
                type="button"
                className="item-picker-pick-set"
                aria-pressed={pickedSets.includes(entry.key)}
                aria-label={t('itemPicker.pickSetLabel', { title: entry.title })}
                onClick={() => {
                  if (entry.key !== 'all') {
                    onPickSet(entry.key);
                  }
                }}
              >
                {t('itemPicker.pickSet')}
              </button>
            )}
          </li>
        );
      })}
    </ul>
  );
};

export interface ResultRowProps {
  readonly id: string;
  readonly item: Item;
  readonly picked: boolean;
  readonly focused: boolean;
  readonly className: string | null;
  readonly setTitles: readonly string[];
  readonly rowRef: Ref<HTMLLIElement>;
  readonly onFocus: () => void;
  readonly onToggle: () => void;
  readonly onKeyDown: (event: KeyboardEvent<HTMLLIElement>) => void;
}

/** One result: thumbnail, title, key values, then the class, the sets and the media count. */
export const ResultRow = ({
  id,
  item,
  picked,
  focused,
  className,
  setTitles,
  rowRef,
  onFocus,
  onToggle,
  onKeyDown,
}: ResultRowProps): ReactElement => {
  const { t } = useTranslator(common, sites);
  const values = keyValues(item);
  const meta = [
    ...(className === null ? [] : [className]),
    ...(setTitles.length === 0 ? [] : [t('itemPicker.inSets', { sets: setTitles.join(', ') })]),
    ...(item.media.length === 0 ? [] : [t('itemPicker.mediaCount', { count: item.media.length })]),
  ];
  return (
    <li
      id={id}
      ref={rowRef}
      role="option"
      className="item-picker-row"
      aria-selected={picked}
      aria-labelledby={`${id}-title`}
      tabIndex={focused ? 0 : -1}
      onFocus={onFocus}
      onClick={onToggle}
      onKeyDown={onKeyDown}
    >
      <Thumb url={item.thumbnailDisplayUrls.square} className="item-picker-thumb" />
      <span className="item-picker-row-text">
        <span id={`${id}-title`} className="item-picker-title">
          {item.title ?? t('common.untitled')}
        </span>
        {values.length === 0 ? null : <span className="item-picker-values">{values.join(' · ')}</span>}
        {meta.length === 0 ? null : <span className="item-picker-meta">{meta.join(' · ')}</span>}
      </span>
      <span className="item-picker-check" aria-hidden="true" />
    </li>
  );
};

/** Rows that hold the place of results still on their way, at the height of a real row. */
export const SkeletonRows = ({ count }: { readonly count: number }): ReactElement => (
  <>
    {Array.from({ length: count }, (_, at) => (
      <li key={`skeleton-${String(at)}`} className="item-picker-row item-picker-skeleton" aria-hidden="true">
        <span className="item-picker-thumb" />
        <span className="item-picker-row-text">
          <span className="item-picker-bar" />
          <span className="item-picker-bar is-short" />
        </span>
      </li>
    ))}
  </>
);

const MediaStrip = ({ item }: { readonly item: Item }): ReactElement | null => {
  const { t } = useTranslator(common, sites);
  const state = useAsync(
    async (): Promise<readonly Media[]> =>
      item.media.length === 0
        ? []
        : (await api.listMediaForItem(item.id, { perPage: PEEK_MEDIA })).items,
    [item.id],
  );
  const media = state.status === 'ready' ? state.data : [];
  if (media.length === 0) {
    return null;
  }
  return (
    <ul className="item-picker-media" aria-label={t('itemPicker.mediaCount', { count: item.media.length })}>
      {media.map((entry) => (
        <li key={String(entry.id)}>
          <Thumb url={entry.thumbnailDisplayUrls.square ?? previewImageUrl(entry)} className="item-picker-media-thumb" />
          <span className="item-picker-hidden">{mediaName(entry) ?? t('common.untitled')}</span>
        </li>
      ))}
    </ul>
  );
};

export interface PeekProps {
  readonly item: Item | null;
  readonly peekRef: Ref<HTMLElement>;
  readonly onLeft: () => void;
}

/** The focused item's values, field by field, its media and a link to its admin page. */
export const Peek = ({ item, peekRef, onLeft }: PeekProps): ReactElement => {
  const { t } = useTranslator(common, sites);
  const rows = item === null ? [] : valueRows(item);
  return (
    <section
      className="item-picker-peek"
      ref={peekRef}
      tabIndex={-1}
      aria-label={t('itemPicker.peek')}
      onKeyDown={(event) => {
        if (event.key === 'ArrowLeft') {
          event.preventDefault();
          onLeft();
        }
      }}
    >
      {item === null ? (
        <p className="field-note">{t('itemPicker.peekEmpty')}</p>
      ) : (
        <>
          <h4>{item.title ?? t('common.untitled')}</h4>
          <MediaStrip item={item} />
          {rows.length === 0 ? (
            <p className="field-note">{t('itemPicker.noValues')}</p>
          ) : (
            <dl>
              {rows.map((row) => (
                <div key={row.term}>
                  <dt>{row.label}</dt>
                  <dd>{row.texts.join('; ')}</dd>
                </div>
              ))}
            </dl>
          )}
          <a href={`#/items/${String(item.id)}`} target="_blank" rel="noreferrer">
            {t('itemPicker.openInAdmin')}
          </a>
        </>
      )}
    </section>
  );
};

export interface TrayProps {
  readonly id: string;
  readonly picked: readonly ItemId[];
  readonly titleOf: (item: ItemId) => string;
  readonly reorder: boolean;
  readonly onChange: (picked: readonly ItemId[]) => void;
  /** `null` when the caller does not pick whole sets. */
  readonly sets: {
    readonly picked: readonly ItemSetId[];
    readonly titleOf: (set: ItemSetId) => string;
    readonly onChange: (sets: readonly ItemSetId[]) => void;
  } | null;
}

/** The picks as chips, in their order, whether or not the results show them. */
export const Tray = ({ id, picked, titleOf, reorder, onChange, sets }: TrayProps): ReactElement => {
  const { t } = useTranslator(common, sites);
  const [dragged, setDragged] = useState<number | null>(null);
  const setPicks = sets?.picked ?? [];
  const count = picked.length + setPicks.length;
  const move = (event: KeyboardEvent<HTMLElement>, at: number): void => {
    if (!event.altKey || (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight')) {
      return;
    }
    event.preventDefault();
    const to = at + (event.key === 'ArrowLeft' ? -1 : 1);
    const next = moved(picked, at, to);
    if (next !== picked) {
      onChange(next);
      const chips = event.currentTarget.closest('ol')?.querySelectorAll<HTMLElement>('.item-picker-chip-title');
      requestAnimationFrame(() => chips?.[to]?.focus());
    }
  };
  return (
    <section className="item-picker-tray" aria-label={t('itemPicker.tray')}>
      <p className="item-picker-tray-head">
        <strong>{t('itemPicker.picked', { count })}</strong>
        {count === 0 ? (
          <span className="field-note">{t('itemPicker.trayEmpty')}</span>
        ) : (
          <button
            type="button"
            className="link-button"
            onClick={() => {
              onChange([]);
              sets?.onChange([]);
            }}
          >
            {t('itemPicker.clear')}
          </button>
        )}
      </p>
      {setPicks.length === 0 || sets === null ? null : (
        <ul className="item-picker-chips">
          {setPicks.map((set) => (
            <li key={`set-${String(set)}`} className="item-picker-chip is-set">
              <span>{t('itemPicker.setChip', { title: sets.titleOf(set) })}</span>
              <button
                type="button"
                aria-label={t('itemPicker.removeSet', { title: sets.titleOf(set) })}
                onClick={() => {
                  sets.onChange(setPicks.filter((entry) => entry !== set));
                }}
              >
                ×
              </button>
            </li>
          ))}
        </ul>
      )}
      {picked.length === 0 ? null : (
        <ol className="item-picker-chips">
          {picked.map((item, at) => (
            <li
              key={String(item)}
              className={dragged === at ? 'item-picker-chip is-dragged' : 'item-picker-chip'}
              draggable={reorder}
              onDragStart={(event) => {
                event.dataTransfer.effectAllowed = 'move';
                setDragged(at);
              }}
              onDragOver={(event) => {
                if (dragged !== null) {
                  event.preventDefault();
                }
              }}
              onDrop={(event) => {
                event.preventDefault();
                if (dragged !== null) {
                  onChange(moved(picked, dragged, at));
                }
                setDragged(null);
              }}
              onDragEnd={() => {
                setDragged(null);
              }}
            >
              <span
                className="item-picker-chip-title"
                tabIndex={reorder ? 0 : undefined}
                aria-describedby={reorder ? `${id}-tray-help` : undefined}
                onKeyDown={
                  reorder
                    ? (event) => {
                        move(event, at);
                      }
                    : undefined
                }
              >
                {titleOf(item)}
              </span>
              <button
                type="button"
                aria-label={t('itemPicker.removePick', { title: titleOf(item) })}
                onClick={() => {
                  onChange(picked.filter((entry) => entry !== item));
                }}
              >
                ×
              </button>
            </li>
          ))}
        </ol>
      )}
      {reorder && picked.length > 1 ? (
        <p id={`${id}-tray-help`} className="field-note">
          {t('itemPicker.trayHelp')}
        </p>
      ) : null}
    </section>
  );
};
