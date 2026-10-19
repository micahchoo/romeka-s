/**
 * One site setting's control: its plain label, the control its kind asks for,
 * and the sentence that says what a visitor sees change. The help is wired to
 * the control with `aria-describedby`, so a screen reader reads it with the
 * label.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { isRecord } from '../../api/parse';
import type { Property } from '../../api/types';
import { useTranslator } from '../../i18n';
import type { KeyOf, MessageKey } from '../../i18n';
import { AssetPicker } from './editor/fields/AssetPicker';
import type { SiteSettingDraft, SiteSettingSpec } from './siteSettingsSchema';
import { sites } from '../../i18n/namespaces/sites';

/** A message id this file names: the shell's or its namespaces'. */
type MessageId = MessageKey | KeyOf<typeof sites>;

const isStringList = (value: unknown): value is readonly string[] =>
  Array.isArray(value) && value.every((item) => typeof item === 'string');

/** The sort fields the browse accepts, each with a readable name. */
const SORT_FIELDS: readonly {
  readonly value: string;
  readonly labelKey: MessageId;
}[] = [
  { value: 'title', labelKey: 'siteSettingSort.title' },
  { value: 'created', labelKey: 'siteSettingSort.created' },
  { value: 'id', labelKey: 'siteSettingSort.id' },
];

/** `asc` and `desc` read as what they do to the field: A to Z, or oldest first. */
const orderLabels = (
  sortBy: string,
): Readonly<Record<'asc' | 'desc', MessageId>> =>
  sortBy === 'title'
    ? { asc: 'siteSettingSort.titleAsc', desc: 'siteSettingSort.titleDesc' }
    : { asc: 'siteSettingSort.dateAsc', desc: 'siteSettingSort.dateDesc' };

const propertyName = (property: Property): string =>
  property.label === null
    ? property.term
    : `${property.label} (${property.term})`;

const matches = (property: Property, query: string): boolean => {
  const needle = query.trim().toLowerCase();
  return (
    needle === '' ||
    property.term.toLowerCase().includes(needle) ||
    (property.label ?? '').toLowerCase().includes(needle)
  );
};

/** A property select narrowed by a search box. The stored term stays an option
 * while the search hides it, and a term the list does not hold is kept. */
const PropertyPicker = ({
  id,
  helpId,
  value,
  properties,
  onChange,
}: {
  readonly id: string;
  readonly helpId: string;
  readonly value: string;
  readonly properties: readonly Property[];
  readonly onChange: (value: string) => void;
}): ReactElement => {
  const { t } = useTranslator(sites);
  const [query, setQuery] = useState('');
  const current = properties.find((property) => property.term === value);
  const shown = properties.filter(
    (property) => property.term === value || matches(property, query),
  );
  return (
    <span className="property-picker">
      <input
        id={`${id}-search`}
        type="search"
        aria-label={t('siteSetting.propertySearch')}
        placeholder={t('siteSetting.propertySearch')}
        value={query}
        onChange={(event) => {
          setQuery(event.target.value);
        }}
      />
      <select
        id={id}
        aria-describedby={helpId}
        value={value}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      >
        <option value="">{t('siteSetting.propertyDefault')}</option>
        {value !== '' && current === undefined ? (
          <option value={value}>
            {t('siteSetting.propertyNotListed', { term: value })}
          </option>
        ) : null}
        {shown.map((property) => (
          <option key={String(property.id)} value={property.term}>
            {propertyName(property)}
          </option>
        ))}
      </select>
    </span>
  );
};

const SortControl = ({
  id,
  spec,
  value,
  onChange,
}: {
  readonly id: string;
  readonly spec: SiteSettingSpec & { readonly kind: 'sort' };
  readonly value: SiteSettingDraft;
  readonly onChange: (value: SiteSettingDraft) => void;
}): ReactElement => {
  const { t } = useTranslator(sites);
  const sortBy =
    isRecord(value) && typeof value.sortBy === 'string'
      ? value.sortBy
      : spec.sortBy;
  const sortOrder =
    isRecord(value) && typeof value.sortOrder === 'string'
      ? value.sortOrder
      : spec.sortOrder;
  const orders = orderLabels(sortBy);
  return (
    <>
      <label htmlFor={`${id}-by`}>{t('sites.sortBy')}</label>
      <select
        id={`${id}-by`}
        value={sortBy}
        onChange={(event) => {
          onChange({ sortBy: event.target.value, sortOrder });
        }}
      >
        {SORT_FIELDS.map((field) => (
          <option key={field.value} value={field.value}>
            {t(field.labelKey)}
          </option>
        ))}
      </select>
      <label htmlFor={`${id}-order`}>{t('sites.sortOrder')}</label>
      <select
        id={`${id}-order`}
        value={sortOrder}
        onChange={(event) => {
          onChange({ sortBy, sortOrder: event.target.value });
        }}
      >
        <option value="asc">{t(orders.asc)}</option>
        <option value="desc">{t(orders.desc)}</option>
      </select>
    </>
  );
};

export const SettingControl = ({
  spec,
  value,
  properties,
  onChange,
}: {
  readonly spec: SiteSettingSpec;
  readonly value: SiteSettingDraft;
  readonly properties: readonly Property[];
  readonly onChange: (value: SiteSettingDraft) => void;
}): ReactElement => {
  const { t } = useTranslator(sites);
  const id = `site-setting-${spec.key}`;
  const helpId = `${id}-help`;
  const label = t(spec.labelKey);
  const help = (
    <p className="setting-help" id={helpId}>
      {t(spec.helpKey)}
    </p>
  );
  const text = typeof value === 'string' ? value : '';

  switch (spec.kind) {
    case 'asset':
      return (
        <AssetPicker
          id={id}
          label={label}
          description={t(spec.helpKey)}
          value={text}
          onChange={onChange}
        />
      );
    case 'multiChoice': {
      const selected = isStringList(value) ? value : [];
      return (
        <fieldset className="site-setting" aria-describedby={helpId}>
          <legend>{label}</legend>
          {help}
          {spec.choices.map((choice) => (
            <label key={choice.value} htmlFor={`${id}-${choice.value}`}>
              <input
                id={`${id}-${choice.value}`}
                type="checkbox"
                checked={selected.includes(choice.value)}
                onChange={(event) => {
                  onChange(
                    event.target.checked
                      ? [...selected, choice.value]
                      : selected.filter((item) => item !== choice.value),
                  );
                }}
              />
              {t(choice.labelKey)}
            </label>
          ))}
        </fieldset>
      );
    }
    case 'sort':
      return (
        <fieldset className="site-setting" aria-describedby={helpId}>
          <legend>{label}</legend>
          {help}
          <SortControl id={id} spec={spec} value={value} onChange={onChange} />
        </fieldset>
      );
    case 'boolean':
      return (
        <div className="site-setting">
          <label htmlFor={id}>
            <input
              id={id}
              type="checkbox"
              aria-describedby={helpId}
              checked={value === true}
              onChange={(event) => {
                onChange(event.target.checked);
              }}
            />
            {label}
          </label>
          {help}
        </div>
      );
    default:
      return (
        <div className="site-setting">
          <label htmlFor={id}>{label}</label>
          {spec.kind === 'property' ? (
            <PropertyPicker
              id={id}
              helpId={helpId}
              value={text}
              properties={properties}
              onChange={onChange}
            />
          ) : spec.kind === 'choice' ? (
            <select
              id={id}
              aria-describedby={helpId}
              value={text}
              onChange={(event) => {
                onChange(event.target.value);
              }}
            >
              {spec.default === null ? (
                <option value="">{t('siteSetting.choiceAutomatic')}</option>
              ) : null}
              {spec.choices.map((choice) => (
                <option key={choice.value} value={choice.value}>
                  {t(choice.labelKey)}
                </option>
              ))}
            </select>
          ) : (
            <input
              id={id}
              type={spec.kind === 'number' ? 'number' : 'text'}
              min={spec.kind === 'number' ? 1 : undefined}
              aria-describedby={helpId}
              value={
                spec.kind === 'number'
                  ? typeof value === 'number'
                    ? String(value)
                    : ''
                  : text
              }
              onChange={(event) => {
                const raw = event.target.value;
                onChange(
                  spec.kind === 'number' && raw !== '' ? Number(raw) : raw,
                );
              }}
            />
          )}
          {help}
        </div>
      );
  }
};
