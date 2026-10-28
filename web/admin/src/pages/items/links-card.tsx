/**
 * The links place: when the Items browse is filtered to the items that link to one record
 * (`links=<id>`, optional `via=<property id>`), that record's card heads the results. Like the
 * set card, the card and the filter are one state, so removing the chip leaves the place.
 */
import type { ReactElement } from 'react';
import { api, ApiError } from '../../api/client';
import { linksQuery, placesApi } from '../../api/places';
import type { RecordSummary } from '../../api/places';
import type { PropertyId } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { recordHref } from '../../app/record-href';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import type { KeyOf, MessageKey, TermLabels, Translator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';
import { useTermLabels } from '../../i18n/termText';
import type { LinksFilter } from './browse';

/** The field the items link through, by term so its label translates. */
interface FieldName {
  readonly term: string;
  readonly label: string | null;
}

export interface LinksFacts {
  readonly record: RecordSummary;
  /** `null` when the filter names no field, or names one the caller cannot read. */
  readonly field: FieldName | null;
  /** `GET /api/items` total for the links filter alone. */
  readonly itemCount: number;
}

export type LinksPlace =
  | { readonly status: 'none' }
  | { readonly status: 'loading' }
  | { readonly status: 'ready'; readonly facts: LinksFacts }
  | { readonly status: 'unavailable' }
  | { readonly status: 'error'; readonly error: Error };

const fieldOf = async (via: PropertyId | null): Promise<FieldName | null> => {
  if (via === null) {
    return null;
  }
  try {
    const property = await api.getProperty(via);
    return { term: property.term, label: property.label };
  } catch {
    // A field that went or never was still filters as the URL says; the card names it by id.
    return null;
  }
};

const loadLinksFacts = async (links: LinksFilter): Promise<LinksFacts> => {
  const [record, field, items] = await Promise.all([
    placesApi.record(links.target),
    fieldOf(links.via),
    api.listItems({ ...linksQuery(links.target, links.via), perPage: 1 }),
  ]);
  return { record, field, itemCount: items.total };
};

/** A record the caller may not read answers like a missing one, so neither tells it exists. */
const isUnavailable = (error: Error): boolean =>
  error instanceof ApiError && (error.status === 403 || error.status === 404);

/** The place for the browse's links filter, read again whenever `revision` moves. */
export const useLinksPlace = (links: LinksFilter | null, revision: number): LinksPlace => {
  const target = links?.target ?? null;
  const via = links?.via ?? null;
  const state = useAsync(
    () => (target === null ? Promise.resolve(null) : loadLinksFacts({ target, via })),
    [target, via, revision],
  );
  if (target === null) {
    return { status: 'none' };
  }
  if (state.status === 'error') {
    return isUnavailable(state.error) ? { status: 'unavailable' } : { status: 'error', error: state.error };
  }
  return state.status === 'ready' && state.data !== null ? { status: 'ready', facts: state.data } : { status: 'loading' };
};

type Translate = Translator<MessageKey | KeyOf<typeof common> | KeyOf<typeof resources>>['t'];

/** The record's name for the chip and the card: its title, or its id while loading or not available. */
export const recordName = (links: LinksFilter, place: LinksPlace, t: Translate): string => {
  const id = links.target;
  if (place.status === 'unavailable') {
    return t('linksPlace.recordUnavailable', { id });
  }
  if (place.status !== 'ready') {
    return t('linksPlace.record', { id });
  }
  return place.facts.record.title ?? t('common.untitled');
};

/** The field's name: its translated label, or its id when it is not known (yet). */
export const fieldName = (links: LinksFilter, place: LinksPlace, text: TermLabels): string | null => {
  if (links.via === null) {
    return null;
  }
  const field = place.status === 'ready' ? place.facts.field : null;
  return field === null ? `#${String(links.via)}` : (text.label(field.term, field.label) ?? field.term);
};

export interface LinksCardProps {
  readonly links: LinksFilter | null;
  readonly place: LinksPlace;
  /** How many of the linking items the other filters let through, or null when nothing else narrows. */
  readonly matching: number | null;
}

export const LinksCard = ({ links, place, matching }: LinksCardProps): ReactElement | null => {
  const { t, number } = useTranslator(common, resources);
  const text = useTermLabels();
  if (links === null || place.status === 'none') {
    return null;
  }
  if (place.status === 'loading') {
    return (
      <section className="set-card links-card" aria-busy="true">
        <p role="status">{t('linksPlace.loading')}</p>
      </section>
    );
  }
  if (place.status === 'unavailable') {
    return (
      <section className="set-card links-card">
        <p role="alert">{t('linksPlace.unavailable')}</p>
      </section>
    );
  }
  if (place.status === 'error') {
    return (
      <section className="set-card links-card">
        <ErrorNotice error={place.error} />
      </section>
    );
  }
  const { record, itemCount } = place.facts;
  const field = fieldName(links, place, text);
  return (
    <section className="set-card links-card" aria-labelledby="links-card-title">
      <h2 id="links-card-title">{t('linksPlace.heading', { title: recordName(links, place, t) })}</h2>
      <p className="set-card-facts">
        <span>{field === null ? t('linksPlace.anyField') : t('linksPlace.via', { field })}</span>
      </p>
      <p className="set-card-count">
        <strong>{itemCount === 1 ? t('setPlace.countOne') : t('setPlace.countMany', { count: number(itemCount) })}</strong>
        {matching === null ? null : (
          <span className="set-card-match">
            {matching === 1 ? t('setPlace.matchOne') : t('setPlace.matchMany', { count: number(matching) })}
          </span>
        )}
      </p>
      <p>
        <a href={recordHref(record)}>{t('linksPlace.open')}</a>
      </p>
    </section>
  );
};
