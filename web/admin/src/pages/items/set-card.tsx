/**
 * The set's place: when the Items browse is filtered to one item set, the set's
 * card heads the results. The card and the filter are one state (`set=` in the
 * URL), so removing the chip leaves the place and the card goes with it.
 */
import type { ReactElement } from 'react';
import { useLayoutEffect, useRef, useState } from 'react';
import type { ItemSet, ItemSetId } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { common } from '../../i18n/namespaces/common';
import { resources } from '../../i18n/namespaces/resources';
import { SetDeleteControl } from '../item-sets/delete';
import { isUnavailable, loadSetFacts, setEditHref } from '../item-sets/facts';
import type { SetFacts } from '../item-sets/facts';

export type SetPlace =
  | { readonly status: 'none' }
  | { readonly status: 'loading' }
  | { readonly status: 'ready'; readonly facts: SetFacts }
  | { readonly status: 'unavailable' }
  | { readonly status: 'error'; readonly error: Error };

/** The place for the browse's set filter, read again whenever `revision` moves. */
export const useSetPlace = (itemSetId: ItemSetId | null, revision: number): SetPlace => {
  const state = useAsync(
    () => (itemSetId === null ? Promise.resolve(null) : loadSetFacts(itemSetId)),
    [itemSetId, revision],
  );
  if (itemSetId === null) {
    return { status: 'none' };
  }
  if (state.status === 'error') {
    return isUnavailable(state.error) ? { status: 'unavailable' } : { status: 'error', error: state.error };
  }
  return state.status === 'ready' && state.data !== null ? { status: 'ready', facts: state.data } : { status: 'loading' };
};

const DESCRIPTION = 'dcterms:description';

const descriptionOf = (set: ItemSet): string | null => {
  const value = set.values.get(DESCRIPTION)?.find((entry) => entry.kind === 'literal');
  return value?.kind === 'literal' && value.text.trim() !== '' ? value.text : null;
};

/** The first three lines, and "Show all" only when the text runs past them. */
const Description = ({ text }: { readonly text: string }): ReactElement => {
  const { t } = useTranslator(resources);
  const [open, setOpen] = useState(false);
  const [overflows, setOverflows] = useState(false);
  const box = useRef<HTMLParagraphElement>(null);
  useLayoutEffect(() => {
    const element = box.current;
    if (element !== null && !open) {
      setOverflows(element.scrollHeight > element.clientHeight + 1);
    }
  }, [text, open]);
  return (
    <div className="set-card-description">
      <p ref={box} className={open ? undefined : 'set-card-clamp'}>
        {text}
      </p>
      {overflows || open ? (
        <button
          type="button"
          className="link-button"
          aria-expanded={open}
          onClick={() => {
            setOpen(!open);
          }}
        >
          {open ? t('setPlace.showLess') : t('setPlace.showAll')}
        </button>
      ) : null}
    </div>
  );
};

export interface SetCardProps {
  readonly place: SetPlace;
  /** How many of the set's items the other filters let through, or null when nothing else narrows. */
  readonly matching: number | null;
}

export const SetCard = ({ place, matching }: SetCardProps): ReactElement | null => {
  const { t, number } = useTranslator(common, resources);
  if (place.status === 'none') {
    return null;
  }
  const crumb = (
    <nav className="set-card-crumb" aria-label={t('setPlace.crumbLabel')}>
      <a href="#/item-sets">{t('page.itemSets')}</a>
    </nav>
  );
  if (place.status === 'loading') {
    return (
      <section className="set-card set-card-loading" aria-busy="true">
        {crumb}
        <p role="status">{t('loading.itemSet')}</p>
      </section>
    );
  }
  if (place.status === 'unavailable') {
    return (
      <section className="set-card">
        {crumb}
        <p role="alert">{t('setPlace.unavailable')}</p>
      </section>
    );
  }
  if (place.status === 'error') {
    return (
      <section className="set-card">
        {crumb}
        <ErrorNotice error={place.error} />
      </section>
    );
  }
  const { set, itemCount, sites } = place.facts;
  const title = set.title ?? t('common.untitled');
  const description = descriptionOf(set);
  return (
    <section className="set-card" aria-labelledby="set-card-title">
      {crumb}
      <h2 id="set-card-title">{title}</h2>
      <p className="set-card-count">
        <strong>{itemCount === 1 ? t('setPlace.countOne') : t('setPlace.countMany', { count: number(itemCount) })}</strong>
        {matching === null ? null : (
          <span className="set-card-match">
            {matching === 1 ? t('setPlace.matchOne') : t('setPlace.matchMany', { count: number(matching) })}
          </span>
        )}
      </p>
      <p className="set-card-facts">
        <span>{set.isPublic ? t('common.public') : t('common.private')}</span>
        <span>{set.isOpen ? t('setPlace.open') : t('setPlace.closed')}</span>
      </p>
      {sites.length === 0 ? (
        <p className="set-card-sites">{t('setPlace.noSites')}</p>
      ) : (
        <p className="set-card-sites">
          {t('setPlace.sites')}{' '}
          {sites.map((site, index) => (
            <span key={site.id}>
              {index === 0 ? null : ', '}
              <a href={`#/sites/${String(site.id)}`}>{site.title}</a>
            </span>
          ))}
        </p>
      )}
      {description === null ? null : <Description text={description} />}
      <div className="set-card-actions">
        <a className="button-link" href={setEditHref(set.id)}>
          {t('setPlace.edit')}
        </a>
        <SetDeleteControl set={set} counts={{ status: 'ready', data: place.facts }} />
      </div>
    </section>
  );
};

/** The empty results under a set with no items: how items get into a set. */
export const SetEmpty = (): ReactElement => {
  const { t } = useTranslator(common, resources);
  return (
    <div className="items-empty set-empty">
      <p>{t('setPlace.empty')}</p>
      <p>
        {t('setPlace.emptyHow', { change: t('items.bulkEdit') })} <a href="#/items">{t('setPlace.toItems')}</a>
      </p>
    </div>
  );
};
