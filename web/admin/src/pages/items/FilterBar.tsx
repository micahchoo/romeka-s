/**
 * The Items browse bar. Search, class, item set and media are always shown;
 * item type, owner, visibility and site wait behind "More filters", which opens
 * itself when one of them is set. Sort and the view toggle close the bar.
 */
import type { ReactElement, ReactNode, Ref } from 'react';
import { useId, useState } from 'react';
import { KIND_FILTERS, KIND_LABEL } from '../../app/media';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';
import { useTermLabels } from '../../i18n/termText';
import { isSet, MORE_KEYS, SORTS } from './browse';
import type { BrowseFilters, ItemBrowse, MediaFilter, SiteFilter, Sort, View } from './browse';
import type { BrowseOptions } from './browse-options';
import type { OnFilter } from './filter-value';

export interface FilterBarProps {
  readonly state: ItemBrowse;
  readonly draft: string;
  readonly searchRef: Ref<HTMLInputElement>;
  readonly options: BrowseOptions;
  readonly onDraft: (text: string) => void;
  /** Escape in the search box: clear it now, without the settle delay. */
  readonly onClearSearch: () => void;
  readonly onFilter: OnFilter;
  readonly onSort: (sort: Sort) => void;
  readonly onView: (view: View) => void;
}

interface Choice {
  readonly value: string;
  readonly label: string;
}

/** The id whose text is `value`, from a map's keys, so a select hands back a typed id. */
const keyOf = <K,>(map: ReadonlyMap<K, unknown>, value: string): K | null =>
  [...map.keys()].find((key) => String(key) === value) ?? null;

const byLabel = (a: Choice, b: Choice): number => a.label.localeCompare(b.label);

const choicesOf = <K,>(map: ReadonlyMap<K, string>): readonly Choice[] =>
  [...map].map(([key, label]) => ({ value: String(key), label })).sort(byLabel);

const SelectFilter = ({
  label,
  value,
  anyLabel,
  choices,
  onChoose,
}: {
  readonly label: string;
  readonly value: string;
  readonly anyLabel: string;
  readonly choices: readonly Choice[];
  readonly onChoose: (value: string) => void;
}): ReactElement => {
  const id = useId();
  return (
    <span className="filter-field">
      <label htmlFor={id}>{label}</label>
      <select
        id={id}
        value={value}
        onChange={(event) => {
          onChoose(event.target.value);
        }}
      >
        <option value="">{anyLabel}</option>
        {choices.map((choice) => (
          <option key={choice.value} value={choice.value}>
            {choice.label}
          </option>
        ))}
      </select>
    </span>
  );
};

const siteValue = (site: SiteFilter | null): string =>
  site === null ? '' : site.kind === 'site' ? String(site.id) : site.kind;

const mediaValue = (media: MediaFilter | null): string =>
  media === null ? '' : media.kind === 'of' ? media.of : media.kind;

const SORT_LABEL = {
  newest: 'browse.sortNewest',
  oldest: 'browse.sortOldest',
  modified: 'browse.sortModified',
  title: 'browse.sortTitle',
} as const satisfies Readonly<Record<Sort, string>>;

/** "More filters" opens itself when one of its filters becomes set, and stays as the reader left it otherwise. */
const useMoreOpen = (filters: BrowseFilters): readonly [boolean, (open: boolean) => void] => {
  const anySet = MORE_KEYS.some((key) => isSet(filters, key));
  const [open, setOpen] = useState(anySet);
  const [wasSet, setWasSet] = useState(anySet);
  if (anySet !== wasSet) {
    setWasSet(anySet);
    if (anySet) {
      setOpen(true);
    }
  }
  return [open, setOpen];
};

const MoreFilters = ({
  filters,
  options,
  onFilter,
}: Pick<FilterBarProps, 'options' | 'onFilter'> & { readonly filters: BrowseFilters }): ReactNode => {
  const { t } = useTranslator(common, resources);
  const any = t('browse.anyValue');
  const siteChoices: readonly Choice[] = [
    { value: 'any', label: t('browse.siteAny') },
    { value: 'none', label: t('browse.siteNone') },
    ...choicesOf(options.sites),
  ];
  return (
    <>
      <SelectFilter
        label={t('browse.template')}
        value={filters.templateId === null ? '' : String(filters.templateId)}
        anyLabel={any}
        choices={choicesOf(options.templates)}
        onChoose={(value) => {
          onFilter({ templateId: keyOf(options.templates, value) });
        }}
      />
      <SelectFilter
        label={t('field.owner')}
        value={filters.ownerId === null ? '' : String(filters.ownerId)}
        anyLabel={any}
        choices={choicesOf(options.users)}
        onChoose={(value) => {
          onFilter({ ownerId: keyOf(options.users, value) });
        }}
      />
      <SelectFilter
        label={t('browse.visibility')}
        value={filters.visibility ?? ''}
        anyLabel={any}
        choices={[
          { value: 'public', label: t('common.public') },
          { value: 'private', label: t('common.private') },
        ]}
        onChoose={(value) => {
          onFilter({ visibility: value === 'public' || value === 'private' ? value : null });
        }}
      />
      <SelectFilter
        label={t('browse.site')}
        value={siteValue(filters.site)}
        anyLabel={any}
        choices={siteChoices}
        onChoose={(value) => {
          const id = keyOf(options.sites, value);
          onFilter({
            site:
              value === 'any' || value === 'none'
                ? { kind: value }
                : id === null
                  ? null
                  : { kind: 'site', id },
          });
        }}
      />
    </>
  );
};

const mediaFilterOf = (value: string): MediaFilter | null => {
  if (value === 'any' || value === 'none') {
    return { kind: value };
  }
  const entry = KIND_FILTERS.find((filter) => filter.kind === value);
  return entry === undefined ? null : { kind: 'of', of: entry.kind };
};

const SortAndView = ({ state, onSort, onView }: Pick<FilterBarProps, 'state' | 'onSort' | 'onView'>): ReactElement => {
  const { t } = useTranslator(resources);
  const sortId = useId();
  return (
    <span className="items-sort-view">
      <span className="filter-field">
        <label htmlFor={sortId}>{t('browse.sort')}</label>
        <select
          id={sortId}
          value={state.sort}
          onChange={(event) => {
            onSort(SORTS.find((sort) => sort === event.target.value) ?? 'newest');
          }}
        >
          {SORTS.map((sort) => (
            <option key={sort} value={sort}>
              {t(SORT_LABEL[sort])}
            </option>
          ))}
        </select>
      </span>
      <span role="group" aria-label={t('browse.view')} className="view-toggle">
        {(['table', 'grid'] as const).map((view) => (
          <button
            key={view}
            type="button"
            aria-pressed={state.view === view}
            onClick={() => {
              onView(view);
            }}
          >
            {t(view === 'table' ? 'browse.viewTable' : 'browse.viewGrid')}
          </button>
        ))}
      </span>
    </span>
  );
};

export const FilterBar = (props: FilterBarProps): ReactElement => {
  const { state, draft, searchRef, options, onDraft, onClearSearch, onFilter } = props;
  const { t } = useTranslator(common, resources);
  const text = useTermLabels();
  const searchId = useId();
  const moreId = useId();
  const [moreOpen, setMoreOpen] = useMoreOpen(state.filters);
  const { filters } = state;
  const classChoices = options.classes
    .map((entry) => ({ value: String(entry.id), label: text.label(entry.term, entry.label ?? entry.term) }))
    .sort(byLabel);
  const mediaChoices: readonly Choice[] = [
    { value: 'any', label: t('browse.mediaAny') },
    { value: 'none', label: t('browse.mediaNone') },
    ...KIND_FILTERS.map((entry) => ({ value: entry.kind, label: t(KIND_LABEL[entry.kind]) })),
  ];
  return (
    <div className="items-filter-bar">
      <span className="filter-field filter-search">
        <label htmlFor={searchId}>{t('browse.search')}</label>
        <input
          id={searchId}
          ref={searchRef}
          type="search"
          value={draft}
          aria-keyshortcuts="/"
          onChange={(event) => {
            onDraft(event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === 'Escape') {
              event.preventDefault();
              onClearSearch();
            }
          }}
        />
      </span>
      <SelectFilter
        label={t('field.class')}
        value={filters.classId === null ? '' : String(filters.classId)}
        anyLabel={t('browse.anyValue')}
        choices={classChoices}
        onChoose={(value) => {
          onFilter({ classId: keyOf(options.classNames, value) });
        }}
      />
      <SelectFilter
        label={t('browse.itemSet')}
        value={filters.itemSetId === null ? '' : String(filters.itemSetId)}
        anyLabel={t('browse.anyValue')}
        choices={choicesOf(options.itemSets)}
        onChoose={(value) => {
          onFilter({ itemSetId: keyOf(options.itemSets, value) });
        }}
      />
      <SelectFilter
        label={t('browse.media')}
        value={mediaValue(filters.media)}
        anyLabel={t('browse.anyValue')}
        choices={mediaChoices}
        onChoose={(value) => {
          onFilter({ media: mediaFilterOf(value) });
        }}
      />
      <button
        type="button"
        className="more-filters-toggle"
        aria-expanded={moreOpen}
        aria-controls={moreId}
        onClick={() => {
          setMoreOpen(!moreOpen);
        }}
      >
        {t('browse.moreFilters')}
      </button>
      <div id={moreId} className="items-more-filters" hidden={!moreOpen}>
        <MoreFilters filters={filters} options={options} onFilter={onFilter} />
      </div>
      <SortAndView state={state} onSort={props.onSort} onView={props.onView} />
    </div>
  );
};
