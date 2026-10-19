import type { ReactElement } from 'react';
import { api } from '../../api/client';
import type { ApiPage, ResourceTemplate } from '../../api/types';
import { ALL_PAGES_PER_PAGE, allPages } from '../../app/all-pages';
import { classLabelOf, classLabels, ownerNameOf, userNames } from '../../app/labels';
import { ListPage, mapPage } from '../../app/list';
import { useTranslator } from '../../i18n';

const PER_PAGE = 25;

interface TemplateRow {
  readonly template: ResourceTemplate;
  readonly classLabel: string | null;
  readonly ownerName: string | null;
}

const load = async (page: number): Promise<ApiPage<TemplateRow>> => {
  const [templates, users, classes] = await Promise.all([
    api.listResourceTemplates({ page, perPage: PER_PAGE, sortBy: 'id', sortOrder: 'asc' }),
    allPages((next) => api.listUsers({ page: next, perPage: ALL_PAGES_PER_PAGE })),
    allPages((next) => api.listResourceClasses({ page: next, perPage: ALL_PAGES_PER_PAGE })),
  ]);
  const names = userNames(users);
  const classNames = classLabels(classes);
  return mapPage(templates, (template) => ({
    template,
    classLabel: classLabelOf(classNames, template.resourceClass),
    ownerName: ownerNameOf(names, template.owner),
  }));
};

/** `Admin/ResourceTemplateController::browseAction`: label, class, owner. */
export const ResourceTemplateList = (): ReactElement => {
  const { t } = useTranslator();
  const resource = t('resource.resourceTemplates');
  return (
    <div>
      <p>
        <a href="#/resource-templates/new">{t('resourceTemplates.add')}</a>
      </p>
      <ListPage<TemplateRow>
        resourceLabel={resource}
        emptyMessage={t('list.empty', { resource })}
        load={load}
        rowKey={(row) => String(row.template.id)}
        columns={[
          {
            header: t('column.label'),
            cell: (row) => (
              <a href={`#/resource-templates/${String(row.template.id)}`}>{row.template.label}</a>
            ),
          },
          { header: t('column.class'), cell: (row) => row.classLabel ?? t('common.none') },
          { header: t('column.owner'), cell: (row) => row.ownerName ?? t('common.noOwner') },
          {
            header: t('column.properties'),
            cell: (row) => row.template.properties.length,
            numeric: true,
          },
        ]}
      />
    </div>
  );
};
