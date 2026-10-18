import type { ReactElement } from 'react';
import { api } from '../../api/client';
import type { ApiPage, ResourceClass } from '../../api/types';
import { vocabularyLabelOf, vocabularyLabels } from '../../app/labels';
import { ListPage, mapPage } from '../../app/list';
import { useTranslator } from '../../i18n';

const PER_PAGE = 100;

interface ClassRow {
  readonly resourceClass: ResourceClass;
  readonly vocabularyLabel: string | null;
}

const load = async (page: number): Promise<ApiPage<ClassRow>> => {
  const [classes, vocabularies] = await Promise.all([
    api.listResourceClasses({ page, perPage: PER_PAGE, sortBy: 'id', sortOrder: 'asc' }),
    api.listVocabularies({ perPage: 100 }),
  ]);
  const labels = vocabularyLabels(vocabularies.items);
  return mapPage(classes, (resourceClass) => ({
    resourceClass,
    vocabularyLabel: vocabularyLabelOf(labels, resourceClass.vocabulary),
  }));
};

/** `Admin/ResourceClassController`: term, label, comment, owning vocabulary. */
export const ResourceClassList = (): ReactElement => {
  const { t } = useTranslator();
  const resource = t('resource.resourceClasses');
  return (
    <div>
      <p>
        <a href="#/resource-classes/new">{t('resourceClasses.add')}</a>
      </p>
      <ListPage<ClassRow>
        resourceLabel={resource}
        emptyMessage={t('list.empty', { resource })}
        load={load}
        rowKey={(row) => String(row.resourceClass.id)}
        columns={[
          {
            header: t('column.term'),
            cell: (row) => (
              <a href={`#/resource-classes/${String(row.resourceClass.id)}`}>
                {row.resourceClass.term}
              </a>
            ),
          },
          { header: t('column.label'), cell: (row) => row.resourceClass.label ?? '' },
          { header: t('column.comment'), cell: (row) => row.resourceClass.comment ?? '' },
          { header: t('column.vocabulary'), cell: (row) => row.vocabularyLabel ?? '' },
        ]}
      />
    </div>
  );
};
