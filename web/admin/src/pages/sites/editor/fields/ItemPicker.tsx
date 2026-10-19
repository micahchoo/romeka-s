/**
 * A picker for several items: search as you type, square thumbnails, many picks.
 *
 * The results are `GET /api/items?search=` (a title match), read once the typing
 * settles. Each result is a toggle button, so Space and Enter pick it; the arrow
 * keys, Home and End move between results. The picked ids live with the caller,
 * so the picker can sit in any form that needs a set of items.
 */
import type { KeyboardEvent, ReactElement } from 'react';
import { useRef, useState } from 'react';
import { api } from '../../../../api/client';
import type { Item, ItemId } from '../../../../api/types';
import { useAsync } from '../../../../app/useAsync';
import { useSettled } from '../../../../app/useSettled';
import { useTranslator } from '../../../../i18n';
import { common } from '../../../../i18n/namespaces/common';
import { sites } from '../../../../i18n/namespaces/sites';
import './ItemPicker.css';

/** How long the picker waits after a key before it searches. */
const SEARCH_DELAY_MS = 300;
const RESULTS = 24;

export interface ItemPickerProps {
  readonly id: string;
  readonly label: string;
  readonly picked: readonly ItemId[];
  readonly onChange: (picked: readonly ItemId[]) => void;
}

/** The result a key moves focus to, or `null` for a key the grid does not handle. */
const nextIndex = (key: string, at: number, count: number): number | null => {
  switch (key) {
    case 'ArrowRight':
    case 'ArrowDown':
      return Math.min(at + 1, count - 1);
    case 'ArrowLeft':
    case 'ArrowUp':
      return Math.max(at - 1, 0);
    case 'Home':
      return 0;
    case 'End':
      return count - 1;
    default:
      return null;
  }
};

export const ItemPicker = ({ id, label, picked, onChange }: ItemPickerProps): ReactElement => {
  const { t } = useTranslator(common, sites);
  const [term, setTerm] = useState('');
  const settled = useSettled(term.trim(), SEARCH_DELAY_MS);
  const results = useAsync(
    async (): Promise<readonly Item[]> =>
      (
        await api.listItems({
          perPage: RESULTS,
          sortBy: 'id',
          sortOrder: 'desc',
          ...(settled === '' ? {} : { search: settled }),
        })
      ).items,
    [settled],
  );
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);

  const toggle = (item: ItemId): void => {
    onChange(picked.includes(item) ? picked.filter((entry) => entry !== item) : [...picked, item]);
  };

  const move = (event: KeyboardEvent<HTMLButtonElement>, at: number, count: number): void => {
    const next = nextIndex(event.key, at, count);
    if (next !== null) {
      event.preventDefault();
      buttons.current[next]?.focus();
    }
  };

  const items = results.status === 'ready' ? results.data : [];
  return (
    <div className="item-picker">
      <label htmlFor={`${id}-search`}>{label}</label>
      <input
        id={`${id}-search`}
        type="search"
        value={term}
        aria-describedby={`${id}-help`}
        onChange={(event) => {
          setTerm(event.target.value);
        }}
      />
      <p id={`${id}-help`} className="field-note">
        {t('itemPicker.help')}
      </p>
      <p className="item-picker-status" role="status">
        {results.status === 'loading'
          ? t('itemPicker.searching')
          : results.status === 'error'
            ? t('itemPicker.error', { message: results.error.message })
            : items.length === 0
              ? t('itemPicker.none')
              : t('itemPicker.picked', { count: picked.length })}
        {picked.length > 0 ? (
          <button
            type="button"
            className="link-button"
            onClick={() => {
              onChange([]);
            }}
          >
            {t('itemPicker.clear')}
          </button>
        ) : null}
      </p>
      <ul className="item-picker-grid" aria-label={t('page.items')}>
        {items.map((item, at) => {
          const title = item.title ?? t('common.untitled');
          const thumbnail = item.thumbnailDisplayUrls.square;
          return (
            <li key={String(item.id)}>
              <button
                type="button"
                ref={(element) => {
                  buttons.current[at] = element;
                }}
                aria-pressed={picked.includes(item.id)}
                onClick={() => {
                  toggle(item.id);
                }}
                onKeyDown={(event) => {
                  move(event, at, items.length);
                }}
              >
                {thumbnail === undefined ? (
                  <span className="item-picker-thumb" aria-hidden="true" />
                ) : (
                  <img className="item-picker-thumb" src={thumbnail} alt="" />
                )}
                <span>{title}</span>
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
};
