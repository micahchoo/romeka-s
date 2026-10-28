/**
 * The active filters, one removable chip each, in words, plus "Clear all". The
 * chips are what tells the reader why the list is shorter than the collection.
 */
import type { ReactElement } from 'react';
import type { ClassId } from '../../api/types';
import { KIND_LABEL } from '../../app/media';
import { useTranslator } from '../../i18n';
import type { KeyOf, Locale, MessageKey, TermLabels, Translator } from '../../i18n';
import { formatDateTime } from '../../app/format';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';
import { useTermLabels } from '../../i18n/termText';
import { activeFilters, chosenId, NONE } from './browse';
import type { BrowseFilters, FilterKey } from './browse';
import { fieldName } from './browse-options';
import type { BrowseOptions } from './browse-options';
import { fieldName as linkFieldName, recordName } from './links-card';
import type { LinksPlace } from './links-card';

type Translate = Translator<MessageKey | KeyOf<typeof common> | KeyOf<typeof resources>>['t'];

const nameIn = <K extends number>(map: ReadonlyMap<K, string>, id: K): string =>
  map.get(id) ?? `#${String(id)}`;

const className = (options: BrowseOptions, id: ClassId, text: TermLabels): string => {
  const name = options.classNames.get(id);
  return name === undefined ? `#${String(id)}` : text.label(name.term, name.label);
};

/** The chip's words for one set filter. */
const chipText = (
  key: FilterKey,
  filters: BrowseFilters,
  options: BrowseOptions,
  t: Translate,
  text: TermLabels,
  linksPlace: LinksPlace,
  locale: Locale,
): string => {
  switch (key) {
    case 'search':
      return t('browse.chipSearch', { text: filters.search });
    case 'classId':
      return t('browse.chipClass', { name: filters.classId === null ? '' : className(options, filters.classId, text) });
    case 'itemSetId': {
      const set = chosenId(filters.itemSetId);
      if (set === null) {
        return filters.itemSetId === NONE ? t('browse.itemSetNone') : '';
      }
      return t('browse.chipItemSet', { name: nameIn(options.itemSets, set) });
    }
    case 'links': {
      if (filters.links === null) {
        return '';
      }
      const name = recordName(filters.links, linksPlace, t);
      // A record that is not available names nothing else, so the chip tells no more than the card.
      const field = linksPlace.status === 'unavailable' ? null : linkFieldName(filters.links, linksPlace, text);
      return field === null ? t('browse.chipLinks', { name }) : t('browse.chipLinksVia', { name, field });
    }
    case 'templateId': {
      const template = chosenId(filters.templateId);
      if (template === null) {
        return filters.templateId === NONE ? t('browse.templateNone') : '';
      }
      return t('browse.chipTemplate', { name: nameIn(options.templates, template) });
    }
    case 'field': {
      if (filters.field === null) {
        return '';
      }
      const field = fieldName(options, chosenId(filters.templateId), filters.field.property, text);
      return t(filters.field.presence === 'empty' ? 'browse.chipFieldEmpty' : 'browse.chipFieldFilled', { field });
    }
    case 'ownerId':
      return t('browse.chipOwner', { name: filters.ownerId === null ? '' : nameIn(options.users, filters.ownerId) });
    case 'visibility':
      return t(filters.visibility === 'private' ? 'common.private' : 'common.public');
    case 'site':
      if (filters.site?.kind === 'site') {
        return t('browse.chipSite', { name: nameIn(options.sites, filters.site.id) });
      }
      return t(filters.site?.kind === 'none' ? 'browse.siteNone' : 'browse.siteAny');
    case 'media':
      if (filters.media?.kind === 'of') {
        return t('browse.chipMedia', { kind: t(KIND_LABEL[filters.media.of]) });
      }
      return t(filters.media?.kind === 'none' ? 'browse.mediaNone' : 'browse.mediaAny');
    case 'changedSince':
      return filters.changedSince === null
        ? ''
        : t('browse.chipChangedByOthers', { date: formatDateTime(filters.changedSince, locale) });
  }
};

export interface FilterChipsProps {
  readonly filters: BrowseFilters;
  readonly options: BrowseOptions;
  /** The record the links filter names, for the chip's words. */
  readonly linksPlace: LinksPlace;
  readonly onRemove: (key: FilterKey) => void;
  readonly onClearAll: () => void;
}

export const FilterChips = ({ filters, options, linksPlace, onRemove, onClearAll }: FilterChipsProps): ReactElement | null => {
  const { t, locale } = useTranslator(common, resources);
  const text = useTermLabels();
  const keys = activeFilters(filters);
  if (keys.length === 0) {
    return null;
  }
  return (
    <div className="filter-chips">
      <ul aria-label={t('browse.activeFilters')}>
        {keys.map((key) => {
          const label = chipText(key, filters, options, t, text, linksPlace, locale);
          return (
            <li key={key} className="filter-chip">
              <span>{label}</span>
              <button
                type="button"
                aria-label={t('browse.removeFilter', { filter: label })}
                onClick={() => {
                  onRemove(key);
                }}
              >
                ×
              </button>
            </li>
          );
        })}
      </ul>
      <button type="button" className="filter-clear-all" onClick={onClearAll}>
        {t('browse.clearAll')}
      </button>
    </div>
  );
};
