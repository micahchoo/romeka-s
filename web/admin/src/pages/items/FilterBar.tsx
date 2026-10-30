/**
 * The Items browse bar. The item type chips lead, then search, item set and media;
 * class, field, owner, visibility and site wait behind "More filters", which opens
 * itself when one of them is set. Sort and the view toggle close the bar.
 */
import type { ReactElement, ReactNode, Ref } from 'react';
import { useId, useState } from 'react';
import { useClassChoices, useClassOptionText } from '../../app/class-choices';
import { KIND_FILTERS, KIND_LABEL } from '../../app/media';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';
import { useTermLabels } from '../../i18n/termText';
import { chosenId, COLUMN_SORTS, isSet, MORE_KEYS, NONE, parseSort, PRESENCES } from './browse';
import type { BrowseFilters, ColumnSort, ItemBrowse, MediaFilter, Presence, SiteFilter, Sort, View } from './browse';
import { fieldChoices } from './browse-options';
import type { BrowseOptions } from './browse-options';
import type { OnFilter } from './filter-value';
import { ItemTypeChips } from './item-type-chips';
import type { ChipCounts } from './item-type-chips';
import { sortFields } from './sort-fields';

export interface FilterBarProps {
  readonly state: ItemBrowse;
  readonly draft: string;
  readonly searchRef: Ref<HTMLInputElement>;
  readonly options: BrowseOptions;
  /** The option lists arrived; until then the item type chips wait. */
  readonly optionsReady: boolean;
  readonly chipCounts: ChipCounts;
  readonly onDraft: (text: string) => void;
  /** Escape in the search box: clear it now, without the settle delay. */
  readonly onClearSearch: () => void;
  readonly onFilter: OnFilter;
  readonly onSort: (sort: Sort) => void;
  readonly onView: (view: View) => void;
  /** Whether the set the list is filtered to has items with an identifier: the sort list then offers it first. */
  readonly setHasIdentifier: boolean;
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
  /** The "no filter" choice; a control that only refines another one has none. */
  readonly anyLabel?: string;
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
        {anyLabel === undefined ? null : <option value="">{anyLabel}</option>}
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
} as const satisfies Readonly<Record<ColumnSort, string>>;

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

const PRESENCE_LABEL = {
  empty: 'browse.fieldEmpty',
  filled: 'browse.fieldFilled',
} as const satisfies Readonly<Record<Presence, string>>;

/** What the second field control offers: empty, has a value, or one exact text. */
type FieldMatch = Presence | 'is';

/** The exact text to look for, sent on Enter or when the box loses focus. */
const ExactTextFilter = ({
  label,
  text,
  onCommit,
}: {
  readonly label: string;
  readonly text: string;
  readonly onCommit: (text: string) => void;
}): ReactElement => {
  const id = useId();
  const [draft, setDraft] = useState(text);
  const commit = (): void => {
    if (draft.trim() !== '' && draft !== text) {
      onCommit(draft);
    }
  };
  return (
    <span className="filter-field">
      <label htmlFor={id}>{label}</label>
      <input
        id={id}
        type="text"
        value={draft}
        onChange={(event) => {
          setDraft(event.target.value);
        }}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === 'Enter') {
            commit();
          }
        }}
      />
    </span>
  );
};

/**
 * A field, then whether it is empty, has a value, or holds one exact text; the second control
 * waits for the first, and "is exactly" waits for its text before it narrows the list.
 */
const FieldFilterControl = ({
  filters,
  options,
  onFilter,
}: Pick<FilterBarProps, 'options' | 'onFilter'> & { readonly filters: BrowseFilters }): ReactNode => {
  const { t } = useTranslator(common, resources);
  const text = useTermLabels();
  const { field } = filters;
  const [askingText, setAskingText] = useState(false);
  const choices = fieldChoices(options, chosenId(filters.templateId), field?.property ?? null, text);
  const match: FieldMatch | null = field === null ? null : askingText ? 'is' : field.presence;
  return (
    <>
      <SelectFilter
        label={t('browse.field')}
        value={field === null ? '' : String(field.property)}
        anyLabel={t('browse.anyValue')}
        choices={choices.map((choice) => ({ value: String(choice.property), label: choice.name }))}
        onChoose={(value) => {
          const property = choices.find((choice) => String(choice.property) === value)?.property ?? null;
          setAskingText(false);
          onFilter({ field: property === null ? null : field === null ? { property, presence: 'empty' } : { ...field, property } });
        }}
      />
      {field === null || match === null ? null : (
        <SelectFilter
          label={t('browse.fieldIs')}
          value={match}
          choices={[
            ...PRESENCES.map((presence) => ({ value: presence, label: t(PRESENCE_LABEL[presence]) })),
            { value: 'is', label: t('browse.fieldIsExactly') },
          ]}
          onChoose={(value) => {
            if (value === 'is') {
              setAskingText(field.presence !== 'is');
              return;
            }
            setAskingText(false);
            onFilter({ field: { property: field.property, presence: value === 'filled' ? 'filled' : 'empty' } });
          }}
        />
      )}
      {field !== null && match === 'is' ? (
        <ExactTextFilter
          key={field.presence === 'is' ? field.text : ''}
          label={t('browse.fieldText')}
          text={field.presence === 'is' ? field.text : ''}
          onCommit={(exact) => {
            setAskingText(false);
            onFilter({ field: { property: field.property, presence: 'is', text: exact } });
          }}
        />
      ) : null}
    </>
  );
};

const MoreFilters = ({
  filters,
  options,
  onFilter,
}: Pick<FilterBarProps, 'options' | 'onFilter'> & { readonly filters: BrowseFilters }): ReactNode => {
  const { t } = useTranslator(common, resources);
  const any = t('browse.anyValue');
  const classes = useClassChoices(filters.classId, 'items', options.usedClasses);
  const classOption = useClassOptionText();
  const siteChoices: readonly Choice[] = [
    { value: 'any', label: t('browse.siteAny') },
    { value: 'none', label: t('browse.siteNone') },
    ...choicesOf(options.sites),
  ];
  return (
    <>
      <SelectFilter
        label={t('field.class')}
        value={filters.classId === null ? '' : String(filters.classId)}
        anyLabel={any}
        choices={classes.map((choice) => ({ value: String(choice.id), label: classOption(choice) }))}
        onChoose={(value) => {
          onFilter({ classId: classes.find((choice) => String(choice.id) === value)?.id ?? null });
        }}
      />
      <FieldFilterControl filters={filters} options={options} onFilter={onFilter} />
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

const SortAndView = ({
  state,
  options,
  setHasIdentifier,
  onSort,
  onView,
}: Pick<FilterBarProps, 'state' | 'options' | 'setHasIdentifier' | 'onSort' | 'onView'>): ReactElement => {
  const { t } = useTranslator(resources);
  const text = useTermLabels();
  const sortId = useId();
  const fields = sortFields(
    options,
    { template: chosenId(state.filters.templateId), chosen: state.sort, setHasIdentifier },
    text,
  );
  const columns = COLUMN_SORTS.map((sort) => (
    <option key={sort} value={sort}>
      {t(SORT_LABEL[sort])}
    </option>
  ));
  const byField =
    fields.length === 0 ? null : (
      <optgroup label={t('browse.sortByField')}>
        {fields.map((field) => (
          <option key={field.term} value={field.term}>
            {field.name}
          </option>
        ))}
      </optgroup>
    );
  /** In a set whose items have identifiers, shelf order is the first thing offered. */
  const fieldsFirst = chosenId(state.filters.itemSetId) !== null && setHasIdentifier;
  return (
    <span className="items-sort-view">
      <span className="filter-field">
        <label htmlFor={sortId}>{t('browse.sort')}</label>
        <select
          id={sortId}
          value={state.sort}
          onChange={(event) => {
            onSort(parseSort(event.target.value));
          }}
        >
          {fieldsFirst ? byField : null}
          {columns}
          {fieldsFirst ? null : byField}
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
  const searchId = useId();
  const moreId = useId();
  const [moreOpen, setMoreOpen] = useMoreOpen(state.filters);
  const { filters } = state;
  const mediaChoices: readonly Choice[] = [
    { value: 'any', label: t('browse.mediaAny') },
    { value: 'none', label: t('browse.mediaNone') },
    ...KIND_FILTERS.map((entry) => ({ value: entry.kind, label: t(KIND_LABEL[entry.kind]) })),
  ];
  return (
    <div className="items-filter-bar">
      <ItemTypeChips
        chosen={filters.templateId}
        options={options}
        optionsReady={props.optionsReady}
        counts={props.chipCounts}
        onFilter={onFilter}
      />
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
        label={t('browse.itemSet')}
        value={filters.itemSetId === null ? '' : String(filters.itemSetId)}
        anyLabel={t('browse.anyValue')}
        choices={[{ value: NONE, label: t('browse.itemSetNone') }, ...choicesOf(options.itemSets)]}
        onChoose={(value) => {
          onFilter({ itemSetId: value === NONE ? NONE : keyOf(options.itemSets, value) });
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
      <SortAndView
        state={state}
        options={options}
        setHasIdentifier={props.setHasIdentifier}
        onSort={props.onSort}
        onView={props.onView}
      />
    </div>
  );
};
