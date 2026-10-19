import type { ReactElement } from 'react';
import { api } from '../../api/client';
import type { ApiPage, ClassId, ResourceClass } from '../../api/types';
import { vocabularyLabelOf, vocabularyLabels } from '../../app/labels';
import { ListPage, mapPage } from '../../app/list';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { DefinedTerm } from '../standards/definition';
import { standardsHref } from '../standards/routes';
import { loadTemplateUsage } from '../standards/usage';

const PER_PAGE = 100;

interface ClassRow {
  readonly resourceClass: ResourceClass;
  readonly vocabularyLabel: string | null;
  readonly templates: number;
}

const load = async (page: number): Promise<ApiPage<ClassRow>> => {
  const [classes, vocabularies, usage] = await Promise.all([
    api.listResourceClasses({ page, perPage: PER_PAGE, sortBy: 'id', sortOrder: 'asc' }),
    api.listVocabularies({ perPage: 100 }),
    loadTemplateUsage(),
  ]);
  const labels = vocabularyLabels(vocabularies.items);
  return mapPage(classes, (resourceClass) => ({
    resourceClass,
    vocabularyLabel: vocabularyLabelOf(labels, resourceClass.vocabulary),
    templates: usage.classes.get(resourceClass.id) ?? 0,
  }));
};

/**
 * The items of one class, from the `resource_class_id` filter's total. Each
 * row asks on its own, so the table draws first and the counts fill in.
 */
const ItemCount = ({ classId }: { readonly classId: ClassId }): ReactElement | null => {
  const { number } = useTranslator();
  const state = useAsync(
    async () => (await api.listItems({ resource_class_id: classId, perPage: 1 })).total,
    [classId],
  );
  return state.status === 'ready' ? <>{number(state.data)}</> : null;
};

/**
 * `Admin/ResourceClassController`: every class of every vocabulary in one
 * table, with the item types and the items that use it.
 */
export const ResourceClassList = (): ReactElement => {
  const { t, number } = useTranslator();
  const resource = t('resource.resourceClasses');
  return (
    <div>
      <p>
        <a href={standardsHref('classes', 'new')}>{t('resourceClasses.add')}</a>
      </p>
      <ListPage<ClassRow>
        resourceLabel={resource}
        emptyMessage={t('list.empty', { resource })}
        load={load}
        rowKey={(row) => String(row.resourceClass.id)}
        columns={[
          {
            header: t('column.term'),
            cell: ({ resourceClass }) => (
              <DefinedTerm term={resourceClass.term} comment={resourceClass.comment}>
                <a href={standardsHref('classes', resourceClass.id)}>{resourceClass.term}</a>
              </DefinedTerm>
            ),
          },
          { header: t('column.label'), cell: (row) => row.resourceClass.label ?? '' },
          { header: t('column.vocabulary'), cell: (row) => row.vocabularyLabel ?? '' },
          { header: t('page.itemTypes'), cell: (row) => number(row.templates), numeric: true },
          {
            header: t('page.items'),
            cell: (row) => <ItemCount classId={row.resourceClass.id} />,
            numeric: true,
          },
        ]}
      />
    </div>
  );
};
