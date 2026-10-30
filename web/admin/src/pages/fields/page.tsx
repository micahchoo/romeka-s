/**
 * `#/fields`: every property, in plain words, with the item types that use it
 * and what they call it. The plain-words search (`app/plain-terms.ts`) finds
 * a field from how people describe it; with no query the list shows the
 * fields in use first, then the common standard ones, then the rest.
 */
import { lazy, Suspense, useId, useState } from 'react';
import type { ReactElement } from 'react';
import { brandId } from '../../api/parse';
import type { Property } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { useLesson } from '../../app/lessons/lesson';
import type { PageProps } from '../../app/router';
import { fieldDescription, plainName, searchTerms } from '../../app/plain-terms';
import { TermHelp } from '../../app/standard-term-picker';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { useTermText } from '../../i18n/termComments';
import '../item-types/item-types.css';
import { useItemTypesText } from '../item-types/messages';
import { standardsHref } from '../standards/routes';
import { common } from '../../i18n/namespaces/common';
import { browseOrder, load, usesOf } from './field-order';
import type { Loaded, Use } from './field-order';
import { valuesHref } from './routes';

const ValuesBoard = lazy(() => import('./values-board').then((module) => ({ default: module.ValuesBoard })));

const FieldEntry = ({
  property,
  uses,
}: {
  readonly property: Property;
  readonly uses: readonly Use[];
}): ReactElement => {
  const t = useItemTypesText();
  const text = useTermText();
  const name = plainName(property, text);
  const renamed = uses.filter((use) => use.label !== null && use.label !== name);
  return (
    <li className="fields-entry">
      <div>
        <p className="fields-entry-head">
          <strong>{name}</strong>
          {renamed.length === 0
            ? null
            : ` ${t('fieldsPage.called', {
                names: renamed
                  .map((use) =>
                    t('fieldsPage.calledIn', { label: use.label ?? '', type: use.template.label }),
                  )
                  .join(', '),
              })}`}
          <TermHelp entry={property} />
        </p>
        <p className="fields-entry-hint">
          {fieldDescription(property, text) ?? ''}
        </p>
        <a className="property-field-term" href={standardsHref('properties', property.id)}>
          {property.term}
        </a>
      </div>
      <p className="fields-entry-use">
        <a className="button-link fields-entry-values" href={valuesHref(property.id)}>
          {t('fieldsPage.values')}
        </a>
        <br />
        {uses.length === 0 ? (
          t('fieldsPage.unused')
        ) : (
          <>
            {t('fieldsPage.used', { count: uses.length })}
            <br />
            {uses.map((use, index) => (
              <span key={String(use.template.id)}>
                {index === 0 ? '' : ', '}
                <a href={`#/resource-templates/${String(use.template.id)}`}>{use.template.label}</a>
              </span>
            ))}
          </>
        )}
      </p>
    </li>
  );
};

const FieldsList = ({ loaded }: { readonly loaded: Loaded }): ReactElement => {
  const t = useItemTypesText();
  const { number } = useTranslator(common);
  const text = useTermText();
  const inputId = useId();
  const [query, setQuery] = useState('');
  const lesson = useLesson('fields-page');
  const shown =
    query.trim() === ''
      ? browseOrder(loaded, text)
      : searchTerms(loaded.properties, query, text, 30);
  return (
    <section className="fields-page">
      <p className="type-lede">
        {t('fieldsPage.lede')} {lesson.toggle}
      </p>
      {lesson.panel}
      <p className="fields-search">
        <label htmlFor={inputId}>{t('fieldsPage.search')}</label>
        <input
          id={inputId}
          type="search"
          placeholder={t('fieldsPage.searchHint')}
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
          }}
        />
      </p>
      <p className="field-note" role="status">
        {query.trim() === ''
          ? t('fieldsPage.count', { count: number(shown.length) })
          : t('fieldsPage.matches', { count: number(shown.length) })}
      </p>
      <ul className="fields-list">
        {shown.map((property) => (
          <FieldEntry
            key={String(property.id)}
            property={property}
            uses={usesOf(loaded.templates, property)}
          />
        ))}
      </ul>
    </section>
  );
};

/** `#/fields` lists the fields; `#/fields/<id>` is one field's values board. */
export const FieldsPage = ({ param }: PageProps): ReactElement => {
  const property = param === null || !/^[1-9]\d*$/.test(param) ? null : brandId<'PropertyId'>(Number(param));
  if (property !== null) {
    return (
      <Suspense fallback={null}>
        <ValuesBoard property={property} />
      </Suspense>
    );
  }
  return <FieldsBrowse />;
};

const FieldsBrowse = (): ReactElement => {
  const { t } = useTranslator(common);
  const state = useAsync(load, []);
  if (state.status === 'loading') {
    return <p role="status">{t('loading.form')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return <FieldsList loaded={state.data} />;
};
