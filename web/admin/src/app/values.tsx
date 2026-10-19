import type { ReactElement } from 'react';
import type { Value } from '../api/types';
import { useTermText, useTranslator } from '../i18n';
import { TableScroll } from './table-scroll';
import { common } from '../i18n/namespaces/common';
import { resources } from '../i18n/namespaces/resources';

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
}

/**
 * The value groups of one resource, one section per property term. Shared by
 * the item and media detail pages.
 */
export const ValuesList = ({ values }: ValuesListProps): ReactElement => {
  const { t } = useTranslator(common, resources);
  const text = useTermText();
  if (values.size === 0) {
    return <p>{t('values.none')}</p>;
  }
  return (
    <>
      {[...values].map(([term, entries]) => (
        <section key={term}>
          <h3>{term}</h3>
          <TableScroll label={t('values.tableLabel', { term })}>
            <table className="table-values">
              <thead>
                <tr>
                  <th>{t('values.property')}</th>
                  <th>{t('values.type')}</th>
                  <th>{t('values.value')}</th>
                  <th>{t('common.public')}</th>
                </tr>
              </thead>
              <tbody>
                {entries.map((entry, index) => (
                  <tr key={`${term}-${String(index)}`}>
                    <td>{text.label(term, entry.propertyLabel)}</td>
                    <td>{entry.type}</td>
                    <td>{renderValue(entry)}</td>
                    <td>{entry.isPublic ? t('common.yes') : t('common.no')}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </TableScroll>
        </section>
      ))}
    </>
  );
};
