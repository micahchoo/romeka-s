import type { ReactElement } from 'react';
import { api } from '../../api/client';
import type { ApiPage, Property } from '../../api/types';
import { vocabularyLabelOf, vocabularyLabels } from '../../app/labels';
import { ListPage, mapPage } from '../../app/list';
import { useTranslator } from '../../i18n';

const PER_PAGE = 100;

interface PropertyRow {
  readonly property: Property;
  readonly vocabularyLabel: string | null;
}

const load = async (page: number): Promise<ApiPage<PropertyRow>> => {
  const [properties, vocabularies] = await Promise.all([
    api.listProperties({ page, perPage: PER_PAGE, sortBy: 'id', sortOrder: 'asc' }),
    api.listVocabularies({ perPage: 100 }),
  ]);
  const labels = vocabularyLabels(vocabularies.items);
  return mapPage(properties, (property) => ({
    property,
    vocabularyLabel: vocabularyLabelOf(labels, property.vocabulary),
  }));
};

/** `Admin/PropertyController`: term, label, comment, owning vocabulary. */
export const PropertyList = (): ReactElement => {
  const { t } = useTranslator();
  const resource = t('resource.properties');
  return (
    <div>
      <p>
        <a href="#/properties/new">{t('properties.add')}</a>
      </p>
      <ListPage<PropertyRow>
        resourceLabel={resource}
        emptyMessage={t('list.empty', { resource })}
        load={load}
        rowKey={(row) => String(row.property.id)}
        columns={[
          {
            header: t('column.term'),
            cell: (row) => <a href={`#/properties/${String(row.property.id)}`}>{row.property.term}</a>,
          },
          { header: t('column.label'), cell: (row) => row.property.label ?? '' },
          { header: t('column.comment'), cell: (row) => row.property.comment ?? '' },
          { header: t('column.vocabulary'), cell: (row) => row.vocabularyLabel ?? '' },
        ]}
      />
    </div>
  );
};
