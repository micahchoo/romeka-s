/**
 * `#/fields`: every property, in plain words, with the item types that use it
 * and what they call it. The plain-words search (`app/plain-terms.ts`) finds
 * a field from how people describe it; with no query the list shows the
 * fields in use first, then the common standard ones, then the rest.
 */
import { useId, useState } from 'react';
import type { ReactElement } from 'react';
import { api } from '../../api/client';
import type { Property, ResourceTemplate } from '../../api/types';
import { ALL_PAGES_PER_PAGE as PER_PAGE, allPages } from '../../app/all-pages';
import { ErrorNotice } from '../../app/errors';
import { PLAIN_TERMS, plainHint, plainName, searchTerms, termComment } from '../../app/plain-terms';
import { TermHelp } from '../../app/standard-term-picker';
import { useAsync } from '../../app/useAsync';
import { useLocale, useTermText, useTranslator, type TermText } from '../../i18n';
import '../item-types/item-types.css';
import { useItemTypesText, WithItemTypesMessages } from '../item-types/messages';
import { standardsHref } from '../standards/routes';

interface Use {
  readonly template: ResourceTemplate;
  /** The type's own name for the field, when it renames it. */
  readonly label: string | null;
}

interface Loaded {
  readonly properties: readonly Property[];
  readonly templates: readonly ResourceTemplate[];
}

const load = async (): Promise<Loaded> => {
  const [properties, templates] = await Promise.all([
    allPages((page) => api.listProperties({ page, perPage: PER_PAGE, sortBy: 'label' })),
    allPages((page) => api.listResourceTemplates({ page, perPage: PER_PAGE, sortBy: 'label' })),
  ]);
  return { properties, templates };
};

const usesOf = (templates: readonly ResourceTemplate[], property: Property): readonly Use[] =>
  templates.flatMap((template) =>
    template.properties
      .filter((row) => row.property.id === property.id)
      .map((row) => ({ template, label: row.alternateLabel })),
  );

/** In use first (most used first), then the curated common fields, then by name. */
const browseOrder = (loaded: Loaded, text: TermText): readonly Property[] => {
  const rank = (property: Property): number =>
    usesOf(loaded.templates, property).length * 10 + (property.term in PLAIN_TERMS ? 1 : 0);
  return [...loaded.properties].sort(
    (a, b) => rank(b) - rank(a) || plainName(a, text).localeCompare(plainName(b, text), text.locale),
  );
};

const FieldEntry = ({
  property,
  uses,
}: {
  readonly property: Property;
  readonly uses: readonly Use[];
}): ReactElement => {
  const t = useItemTypesText();
  const locale = useLocale();
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
          {plainHint(property.term, locale) ?? termComment(property, text) ?? ''}
        </p>
        <a className="property-field-term" href={standardsHref('properties', property.id)}>
          {property.term}
        </a>
      </div>
      <p className="fields-entry-use">
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
  const { number } = useTranslator();
  const text = useTermText();
  const inputId = useId();
  const [query, setQuery] = useState('');
  const shown =
    query.trim() === ''
      ? browseOrder(loaded, text)
      : searchTerms(loaded.properties, query, text, 30);
  return (
    <section className="fields-page">
      <p className="type-lede">{t('fieldsPage.lede')}</p>
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

export const FieldsPage = (): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(load, []);
  if (state.status === 'loading') {
    return <p role="status">{t('loading.form')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return (
    <WithItemTypesMessages>
      <FieldsList loaded={state.data} />
    </WithItemTypesMessages>
  );
};
