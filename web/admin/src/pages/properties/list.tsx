import type { ReactElement } from 'react';
import { api } from '../../api/client';
import type { ApiPage, Property } from '../../api/types';
import { vocabularyLabels } from '../../app/labels';
import type { TermName } from '../../app/labels';
import { ListPage, mapPage } from '../../app/list';
import { useTermText, useTranslator } from '../../i18n';
import { DefinedTerm } from '../standards/definition';
import { standardsHref } from '../standards/routes';
import { loadTemplateUsage } from '../standards/usage';

const PER_PAGE = 100;

interface PropertyRow {
  readonly property: Property;
  readonly vocabulary: TermName | null;
  readonly templates: number;
}

const load = async (page: number): Promise<ApiPage<PropertyRow>> => {
  const [properties, vocabularies, usage] = await Promise.all([
    api.listProperties({ page, perPage: PER_PAGE, sortBy: 'id', sortOrder: 'asc' }),
    api.listVocabularies({ perPage: 100 }),
    loadTemplateUsage(),
  ]);
  const labels = vocabularyLabels(vocabularies.items);
  return mapPage(properties, (property) => ({
    property,
    vocabulary: labels.get(property.vocabulary.id) ?? null,
    templates: usage.properties.get(property.id) ?? 0,
  }));
};

/**
 * `Admin/PropertyController`: every property of every vocabulary in one table,
 * with the number of item types that use it. The API counts no values per
 * property, so the table does not show one.
 */
export const PropertyList = (): ReactElement => {
  const { t, number } = useTranslator();
  const text = useTermText();
  const resource = t('resource.properties');
  return (
    <div>
      <p>
        <a href={standardsHref('properties', 'new')}>{t('properties.add')}</a>
      </p>
      <ListPage<PropertyRow>
        resourceLabel={resource}
        emptyMessage={t('list.empty', { resource })}
        load={load}
        rowKey={(row) => String(row.property.id)}
        columns={[
          {
            header: t('column.term'),
            cell: ({ property }) => (
              <DefinedTerm term={property.term} comment={text.comment(property.term, property.comment)}>
                <a href={standardsHref('properties', property.id)}>{property.term}</a>
              </DefinedTerm>
            ),
          },
          {
            header: t('column.label'),
            cell: (row) => text.label(row.property.term, row.property.label) ?? '',
          },
          {
            header: t('column.vocabulary'),
            cell: ({ vocabulary }) => (vocabulary === null ? '' : text.label(vocabulary.term, vocabulary.label)),
          },
          { header: t('page.itemTypes'), cell: (row) => number(row.templates), numeric: true },
        ]}
      />
    </div>
  );
};
