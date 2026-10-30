/**
 * `#/values`: the way into the values board from the nav. With no field it lists the fields the
 * item types use, each opening its board, and a plain-words search finds any other field;
 * `#/values/<id>` is that field's board.
 */
import { lazy, Suspense, useId, useState } from 'react';
import type { ReactElement } from 'react';
import { brandId } from '../../api/parse';
import type { Property } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { plainName, searchTerms } from '../../app/plain-terms';
import type { PageProps } from '../../app/router';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { fieldValues } from '../../i18n/namespaces/field-values';
import { useTermText } from '../../i18n/termComments';
import { browseOrder, load, usesOf } from './field-order';
import type { Loaded } from './field-order';
import { valuesHref } from './routes';
import './values-board.css';

const ValuesBoard = lazy(() => import('./values-board').then((module) => ({ default: module.ValuesBoard })));

const FieldLink = ({ property, loaded }: { readonly property: Property; readonly loaded: Loaded }): ReactElement => {
  const { t } = useTranslator(fieldValues);
  const text = useTermText();
  const uses = usesOf(loaded.templates, property).length;
  return (
    <li className="values-choice">
      <a href={valuesHref(property.id)}>
        {plainName(property, text)} <span className="values-note">{property.term}</span>
      </a>
      {uses === 0 ? null : (
        <span className="values-note"> · {uses === 1 ? t('values.usedByOne') : t('values.usedBy', { count: String(uses) })}</span>
      )}
    </li>
  );
};

const Chooser = ({ loaded }: { readonly loaded: Loaded }): ReactElement => {
  const { t } = useTranslator(fieldValues);
  const text = useTermText();
  const searchId = useId();
  const usedId = useId();
  const [query, setQuery] = useState('');
  const used = browseOrder(loaded, text).filter((property) => usesOf(loaded.templates, property).length > 0);
  const found = query.trim() === '' ? [] : searchTerms(loaded.properties, query, text, 20);
  return (
    <section className="values-board">
      <p className="type-lede">{t('values.chooseIntro')}</p>
      <h2 id={usedId}>{t('values.inUse')}</h2>
      {used.length === 0 ? (
        <p className="values-empty">{t('values.noneInUse')}</p>
      ) : (
        <ul className="values-choices" aria-labelledby={usedId}>
          {used.map((property) => (
            <FieldLink key={String(property.id)} property={property} loaded={loaded} />
          ))}
        </ul>
      )}
      <p className="values-search">
        <label htmlFor={searchId}>{t('values.findOther')}</label>
        <input
          id={searchId}
          type="search"
          placeholder={t('values.findOtherHint')}
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
          }}
        />
      </p>
      {query.trim() === '' ? null : found.length === 0 ? (
        <p className="values-empty">{t('values.noFieldMatch', { text: query.trim() })}</p>
      ) : (
        <ul className="values-choices" aria-label={t('values.matches', { text: query.trim() })}>
          {found.map((property) => (
            <FieldLink key={String(property.id)} property={property} loaded={loaded} />
          ))}
        </ul>
      )}
    </section>
  );
};

const ChooserPage = (): ReactElement => {
  const { t } = useTranslator(fieldValues);
  const state = useAsync(load, []);
  if (state.status === 'loading') return <p role="status">{t('values.loading')}</p>;
  if (state.status === 'error') return <ErrorNotice error={state.error} />;
  return <Chooser loaded={state.data} />;
};

export const ValuesPage = ({ param }: PageProps): ReactElement => {
  const property = param === null || !/^[1-9]\d*$/.test(param) ? null : brandId<'PropertyId'>(Number(param));
  return (
    <Suspense fallback={null}>
      {property === null ? <ChooserPage /> : <ValuesBoard property={property} />}
    </Suspense>
  );
};
