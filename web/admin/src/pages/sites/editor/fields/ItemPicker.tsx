/**
 * The item picker: a browser over the archive in three columns, the sources
 * (every item, then each item set with its count), the results as rows, and a
 * peek of the focused item, with a tray of the picks below.
 *
 * Every row and the peek read the `/api/items` list response, which carries the
 * values, the class, the sets and the media of each item, so a row costs no
 * request of its own. The search box matches any value, as the API's `search`
 * does. The results grow by "Show more", so rows picked on an earlier page stay
 * in view, and the tray shows every pick whether the results show it or not.
 *
 * The picks live with the caller (`ItemPickerProps`): several in order, or one.
 * This module is its own chunk; callers load it through `LazyItemPicker`.
 */
import type { KeyboardEvent, ReactElement } from 'react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { api } from '../../../../api/client';
import type { ClassId, Item, ItemId, ItemSet, ItemSetId } from '../../../../api/types';
import { KIND_LABEL } from '../../../../app/media';
import { useAsync } from '../../../../app/useAsync';
import { useSettled } from '../../../../app/useSettled';
import { useTranslator } from '../../../../i18n';
import { common } from '../../../../i18n/namespaces/common';
import { resources } from '../../../../i18n/namespaces/resources';
import { sites } from '../../../../i18n/namespaces/sites';
import type { FieldAccepts } from '../../blockSchema';
import type { Filters, ItemPickerProps, MediaNarrowing, Source } from './itemPickerModel';
import {
  ALL_ITEMS,
  HAS_CHOICES,
  NO_FILTERS,
  NO_NARROWING,
  classTerm,
  hasChoiceOf,
  isFiltered,
  itemQuery,
  narrowingFor,
  pickedList,
  toggled,
} from './itemPickerModel';
import { Peek, ResultRow, SkeletonRows, Sources, Tray } from './ItemPickerParts';
import { PAGE_SIZE, useItemListing } from './useItemListing';
import './ItemPicker.css';

/** How long the picker waits after a key before it searches. */
const SEARCH_DELAY_MS = 300;
/** How long the results may take before skeleton rows hold their place. */
const SLOW_MS = 1000;
const SET_LIMIT = 100;
const CLASS_LIMIT = 500;
const SKELETON_ROWS = 6;

/** `true` once `on` has held for `delay`, and `false` as soon as it drops. */
const useDelayed = (on: boolean, delay: number): boolean => {
  const [late, setLate] = useState(false);
  useEffect(() => {
    if (!on) {
      setLate(false);
      return undefined;
    }
    const timer = setTimeout(() => {
      setLate(true);
    }, delay);
    return () => {
      clearTimeout(timer);
    };
  }, [on, delay]);
  return late;
};

/** The item count of every source, read once the sets are known. */
const useSourceCounts = (sets: readonly ItemSet[]): ReadonlyMap<ItemSetId | 'all', number> => {
  const key = sets.map((set) => String(set.id)).join(',');
  const counts = useAsync(async () => {
    const totals = await Promise.all([
      api.listItems({ perPage: 1 }).then((page) => ['all', page.total] as const),
      ...sets.map((set) =>
        api.listItems({ itemSetId: set.id, perPage: 1 }).then((page) => [set.id, page.total] as const),
      ),
    ]);
    return new Map<ItemSetId | 'all', number>(totals);
  }, [key]);
  return counts.status === 'ready' ? counts.data : new Map();
};

/** Every item read so far, by id, plus the picks no result has shown yet. */
const useKnownItems = (rows: readonly Item[], picked: readonly ItemId[]): ReadonlyMap<ItemId, Item> => {
  const seen = useRef(new Map<ItemId, Item>());
  for (const row of rows) {
    seen.current.set(row.id, row);
  }
  const missing = picked.filter((item) => !seen.current.has(item));
  const missingKey = missing.join(',');
  const fetched = useAsync(
    async (): Promise<readonly Item[]> =>
      missing.length === 0 ? [] : (await api.listItems({ id: missingKey, perPage: missing.length })).items,
    [missingKey],
  );
  if (fetched.status === 'ready') {
    for (const item of fetched.data) {
      seen.current.set(item.id, item);
    }
  }
  return seen.current;
};

/**
 * The controls that narrow the archive by media: the plain "Has media" test for
 * a field that names items, "Only items with images" for a field that shows an
 * image, and a "Has" select for a field that takes any media.
 */
const MediaFilter = ({
  id,
  accepts,
  narrowing,
  onChange,
}: {
  readonly id: string;
  readonly accepts: FieldAccepts | undefined;
  readonly narrowing: MediaNarrowing;
  readonly onChange: (narrowing: MediaNarrowing) => void;
}): ReactElement => {
  const { t } = useTranslator(common, sites, resources);
  if (accepts === undefined) {
    return (
      <label className="item-picker-media-filter" htmlFor={`${id}-has-media`}>
        <input
          id={`${id}-has-media`}
          type="checkbox"
          checked={narrowing.kind === 'media'}
          onChange={(event) => {
            onChange(event.target.checked ? { kind: 'media' } : NO_NARROWING);
          }}
        />
        {t('itemPicker.hasMedia')}
      </label>
    );
  }
  if (accepts === 'any') {
    return (
      <p>
        <label htmlFor={`${id}-has`}>{t('itemPicker.has')}</label>
        <select
          id={`${id}-has`}
          value={hasChoiceOf(narrowing).word ?? ''}
          onChange={(event) => {
            const choice = HAS_CHOICES.find((entry) => (entry.word ?? '') === event.target.value);
            onChange(
              choice === undefined || choice.kinds.length === 0
                ? NO_NARROWING
                : { kind: 'kinds', kinds: choice.kinds },
            );
          }}
        >
          {HAS_CHOICES.map((choice) => (
            <option key={choice.word ?? 'any'} value={choice.word ?? ''}>
              {choice.word === null ? t('itemPicker.hasAny') : t(KIND_LABEL[choice.word])}
            </option>
          ))}
        </select>
      </p>
    );
  }
  return (
    <label className="item-picker-media-filter" htmlFor={`${id}-only-images`}>
      <input
        id={`${id}-only-images`}
        type="checkbox"
        checked={narrowing.kind === 'kinds'}
        onChange={(event) => {
          onChange(event.target.checked ? narrowingFor(accepts) : NO_NARROWING);
        }}
      />
      {t('itemPicker.onlyImages')}
    </label>
  );
};

export const ItemPicker = (props: ItemPickerProps): ReactElement => {
  const { id, label, accepts } = props;
  const { t, number } = useTranslator(common, sites, resources);
  const [term, setTerm] = useState('');
  const [source, setSource] = useState<Source>(ALL_ITEMS);
  const [classId, setClassId] = useState<ClassId | null>(null);
  const [narrowing, setNarrowing] = useState<MediaNarrowing>(() =>
    accepts === undefined ? NO_NARROWING : narrowingFor(accepts),
  );
  const search = useSettled(term.trim(), SEARCH_DELAY_MS);
  const filters: Filters = { source, search, classId, narrowing };
  const query = useMemo(
    () => itemQuery({ source, search, classId, narrowing }),
    [source, search, classId, narrowing],
  );
  const { listing, more, retry } = useItemListing(query);
  const slow = useDelayed(listing.status === 'loading', SLOW_MS);

  const setsState = useAsync(() => api.listItemSets({ perPage: SET_LIMIT, sortBy: 'title' }), []);
  const sets = setsState.status === 'ready' ? setsState.data.items : [];
  const counts = useSourceCounts(sets);
  const classesState = useAsync(
    () => api.listResourceClasses({ perPage: CLASS_LIMIT, sortBy: 'label' }),
    [],
  );
  const classes = classesState.status === 'ready' ? classesState.data.items : [];

  const picked = pickedList(props);
  const setPicks = props.mode === 'many' ? props.sets : undefined;
  const known = useKnownItems(listing.rows, picked);
  const rows = listing.rows;
  const [focus, setFocus] = useState(0);
  const at = Math.min(focus, Math.max(rows.length - 1, 0));
  const rowRefs = useRef<(HTMLLIElement | null)[]>([]);
  const sourceRefs = useRef(new Map<ItemSetId | 'all', HTMLButtonElement | null>());
  const peekRef = useRef<HTMLElement | null>(null);

  const emit = (next: readonly ItemId[]): void => {
    if (props.mode === 'many') {
      props.onChange(next);
    } else {
      props.onChange(next[0] ?? null);
    }
  };
  const pick = (item: ItemId): void => {
    emit(toggled(picked, item, props.mode));
  };
  const pickSet = (set: ItemSetId): void => {
    if (setPicks !== undefined) {
      const current = setPicks.picked;
      setPicks.onChange(
        current.includes(set) ? current.filter((entry) => entry !== set) : [...current, set],
      );
    }
  };
  const focusRow = (index: number): void => {
    setFocus(index);
    rowRefs.current[index]?.focus();
  };
  const sourceKey = (of: Source): ItemSetId | 'all' => (of.kind === 'all' ? 'all' : of.id);
  const chooseSource = (next: Source): void => {
    setSource(next);
    setFocus(0);
    requestAnimationFrame(() => {
      sourceRefs.current.get(sourceKey(next))?.focus();
    });
  };
  const clearFilters = (): void => {
    setTerm('');
    setSource(NO_FILTERS.source);
    setClassId(NO_FILTERS.classId);
    setNarrowing(NO_FILTERS.narrowing);
  };

  const onRowKey = (event: KeyboardEvent<HTMLLIElement>, index: number, item: Item): void => {
    const last = rows.length - 1;
    const moves: Readonly<Partial<Record<string, number>>> = {
      ArrowDown: Math.min(index + 1, last),
      ArrowUp: Math.max(index - 1, 0),
      Home: 0,
      End: last,
    };
    const target = moves[event.key];
    if (target !== undefined) {
      event.preventDefault();
      focusRow(target);
    } else if (event.key === ' ' || event.key === 'Enter') {
      event.preventDefault();
      pick(item.id);
    } else if (event.key === 'ArrowRight') {
      event.preventDefault();
      peekRef.current?.focus();
    } else if (event.key === 'ArrowLeft') {
      event.preventDefault();
      sourceRefs.current.get(sourceKey(source))?.focus();
    }
  };

  const setTitle = new Map(sets.map((set) => [set.id, set.title ?? t('common.untitled')]));
  const classLabel = new Map(classes.map((entry) => [entry.id, entry.label ?? entry.term]));
  const titleOf = (item: ItemId): string => {
    const found = known.get(item);
    return found === undefined
      ? t('itemPicker.itemFallback', { id: item })
      : (found.title ?? t('common.untitled'));
  };
  const total = listing.total;
  const loading = listing.status === 'loading';
  const empty = listing.status === 'ready' && total === 0;
  const filtered = isFiltered(filters);

  return (
    <fieldset className="item-picker">
      <legend>{label}</legend>
      <div className="item-picker-filters">
        <p className="item-picker-search">
          <label htmlFor={`${id}-search`}>{t('itemPicker.search')}</label>
          <input
            id={`${id}-search`}
            type="search"
            value={term}
            aria-describedby={`${id}-help`}
            onChange={(event) => {
              setTerm(event.target.value);
              setFocus(0);
            }}
          />
        </p>
        <p>
          <label htmlFor={`${id}-class`}>{t('itemPicker.class')}</label>
          <select
            id={`${id}-class`}
            value={classId === null ? '' : String(classId)}
            onChange={(event) => {
              setClassId(classes.find((entry) => String(entry.id) === event.target.value)?.id ?? null);
              setFocus(0);
            }}
          >
            <option value="">{t('itemPicker.anyClass')}</option>
            {classes.map((entry) => (
              <option key={String(entry.id)} value={String(entry.id)}>
                {entry.label ?? entry.term}
              </option>
            ))}
          </select>
        </p>
        <MediaFilter
          id={id}
          accepts={accepts}
          narrowing={narrowing}
          onChange={(next) => {
            setNarrowing(next);
            setFocus(0);
          }}
        />
      </div>
      <p id={`${id}-help`} className="field-note">
        {t('itemPicker.help')}
      </p>
      <div className="item-picker-columns">
        <Sources
          sets={sets}
          counts={counts}
          current={source}
          pickedSets={setPicks === undefined ? null : setPicks.picked}
          onSource={chooseSource}
          onPickSet={pickSet}
          sourceRef={(key) => (element) => {
            sourceRefs.current.set(key, element);
          }}
          onRight={() => {
            focusRow(at);
          }}
        />
        <section className="item-picker-results" aria-label={t('itemPicker.results')}>
          <p className="item-picker-status" role="status">
            {total === null
              ? loading
                ? t('itemPicker.searching')
                : null
              : total === 1
                ? t('itemPicker.totalOne')
                : t('itemPicker.total', { count: number(total) })}
          </p>
          {listing.status === 'error' ? (
            <div className="item-picker-problem" role="alert">
              <p>{t('itemPicker.error')}</p>
              <button type="button" onClick={retry}>
                {t('itemPicker.retry')}
              </button>
            </div>
          ) : null}
          {empty ? (
            <div className="item-picker-problem">
              <p>{filtered ? t('itemPicker.none') : t('itemPicker.noItems')}</p>
              {filtered ? (
                <button type="button" onClick={clearFilters}>
                  {t('itemPicker.clearFilters')}
                </button>
              ) : null}
            </div>
          ) : null}
          <ul
            className="item-picker-rows"
            role="listbox"
            aria-label={label}
            aria-multiselectable={props.mode === 'many'}
            aria-busy={loading}
          >
            {rows.map((item, index) => (
              <ResultRow
                key={String(item.id)}
                id={`${id}-row-${String(item.id)}`}
                item={item}
                picked={picked.includes(item.id)}
                focused={index === at}
                className={
                  item.resourceClass === null
                    ? null
                    : (classLabel.get(item.resourceClass.id) ?? classTerm(item))
                }
                setTitles={item.itemSets.flatMap((ref) => {
                  const title = setTitle.get(ref.id);
                  return title === undefined ? [] : [title];
                })}
                rowRef={(element) => {
                  rowRefs.current[index] = element;
                }}
                onFocus={() => {
                  setFocus(index);
                }}
                onToggle={() => {
                  pick(item.id);
                }}
                onKeyDown={(event) => {
                  onRowKey(event, index, item);
                }}
              />
            ))}
            {loading && slow ? <SkeletonRows count={SKELETON_ROWS} /> : null}
          </ul>
          {total === null || rows.length === 0 ? null : (
            <p className="item-picker-pager">
              <span>{t('itemPicker.shown', { shown: number(rows.length), total: number(total) })}</span>
              {rows.length < total ? (
                <button type="button" disabled={loading} onClick={more}>
                  {t('itemPicker.more', { count: Math.min(PAGE_SIZE, total - rows.length) })}
                </button>
              ) : null}
            </p>
          )}
        </section>
        <Peek
          item={rows[at] ?? null}
          peekRef={peekRef}
          onLeft={() => {
            focusRow(at);
          }}
        />
      </div>
      <Tray
        id={id}
        picked={picked}
        titleOf={titleOf}
        reorder={props.mode === 'many'}
        onChange={emit}
        sets={
          setPicks === undefined
            ? null
            : {
                picked: setPicks.picked,
                titleOf: (set) => setTitle.get(set) ?? t('common.untitled'),
                onChange: setPicks.onChange,
              }
        }
      />
    </fieldset>
  );
};
