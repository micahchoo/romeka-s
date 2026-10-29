import type { ReactElement } from 'react';
import type { DataType, Value } from '../api/types';
import { useTranslator } from '../i18n';
import type { TermLabels } from '../i18n/termText';
import { useTermLabels } from '../i18n/termText';
import type { TemplateLayout } from './value-fields';
import { valueKindWord } from './value-kind';
import { common } from '../i18n/namespaces/common';
import { resources } from '../i18n/namespaces/resources';

/** One field of the read list: the name the form gives it, its term, its values. */
interface ReadField {
  readonly term: string;
  readonly name: string;
  readonly values: readonly Value[];
}

const nonEmpty = (text: string | null): string | null => (text === null || text.trim() === '' ? null : text);

/**
 * The fields in the form's order and under the form's names (`layoutFields`): the template's
 * rows first, by its alternate label, then each other field by its label in the locale.
 */
const readFields = (
  values: ReadonlyMap<string, readonly Value[]>,
  template: TemplateLayout | null,
  text: TermLabels,
): readonly ReadField[] => {
  const rows = template?.properties ?? [];
  const fields = [...values].map(([term, entries]) => {
    const propertyId = entries[0]?.propertyId;
    const row = rows.findIndex((entry) => entry.property.id === propertyId);
    const name =
      nonEmpty(rows[row]?.alternateLabel ?? null) ?? nonEmpty(text.label(term, entries[0]?.propertyLabel ?? null)) ?? term;
    return { field: { term, name, values: entries }, order: row === -1 ? rows.length : row };
  });
  return fields.sort((a, b) => a.order - b.order).map(({ field }) => field);
};

const renderValue = (value: Value): ReactElement => {
  switch (value.kind) {
    case 'literal':
      return (
        <>
          {value.text}
          {value.lang === null ? null : <span className="value-lang"> ({value.lang})</span>}
        </>
      );
    case 'uri':
      return (
        <a href={value.uri} target="_blank" rel="noreferrer">
          {value.label ?? value.uri}
        </a>
      );
    case 'resource':
      return (
        <span>
          {value.displayTitle ?? `#${String(value.resourceId)}`} ({value.resourceName})
        </span>
      );
    case 'other':
      return <span>[{value.type}]</span>;
  }
};

export interface ValuesListProps {
  readonly values: ReadonlyMap<string, readonly Value[]>;
  /** The resource's item type, whose names for its fields lead; null names each by its label. */
  readonly template?: TemplateLayout | null;
  /** The registered data types, which name a module's value kind; empty names it by its id. */
  readonly dataTypes?: readonly DataType[];
}

/**
 * The values of one resource as one list, each field under the name the form gives it with its
 * term as quiet text. Shared by the item and media pages.
 */
export const ValuesList = ({ values, template = null, dataTypes = [] }: ValuesListProps): ReactElement => {
  const { t } = useTranslator(common, resources);
  const text = useTermLabels();
  if (values.size === 0) {
    return <p>{t('values.none')}</p>;
  }
  return (
    <dl className="value-list">
      {readFields(values, template, text).map((field) => (
        <div className="value-field" key={field.term}>
          <dt>
            <span className="value-field-name">{field.name}</span>
            {field.name === field.term ? null : <span className="value-field-term">{field.term}</span>}
          </dt>
          {field.values.map((value, index) => (
            <dd key={index} className="value-entry">
              <span className="value-text">{renderValue(value)}</span>
              <span className="value-meta">
                <span>{valueKindWord(value.type, dataTypes, t)}</span>
                {value.isPublic ? null : <span className="value-private">{t('common.private')}</span>}
              </span>
            </dd>
          ))}
        </div>
      ))}
    </dl>
  );
};
