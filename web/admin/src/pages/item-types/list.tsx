/**
 * `Admin/ResourceTemplateController::browseAction` as "Item types": each type
 * with its field count, how many items use it, and its standard class.
 */
import type { ReactElement } from 'react';
import { api } from '../../api/client';
import { installation } from '../../app/installation';
import type { ApiPage, ResourceClass, ResourceTemplate } from '../../api/types';
import { ALL_PAGES_PER_PAGE, allPages } from '../../app/all-pages';
import { ListPage, mapPage } from '../../app/list';
import { plainName } from '../../app/plain-terms';
import { useTranslator } from '../../i18n';
import { useTermLabels } from '../../i18n/termText';
import { useItemTypesText } from './messages';

interface TypeRow {
  readonly template: ResourceTemplate;
  readonly resourceClass: ResourceClass | null;
  readonly items: number;
}

const load = async (page: number): Promise<ApiPage<TypeRow>> => {
  const [templates, classes] = await Promise.all([
    api.listResourceTemplates({ page, perPage: installation().perPage, sortBy: 'label', sortOrder: 'asc' }),
    allPages((next) => api.listResourceClasses({ page: next, perPage: ALL_PAGES_PER_PAGE })),
  ]);
  const counts = await Promise.all(
    templates.items.map(
      async (template) =>
        (await api.listItems({ resourceTemplateId: template.id, perPage: 1 })).total,
    ),
  );
  return mapPage(templates, (template) => ({
    template,
    resourceClass: classes.find((entry) => entry.id === template.resourceClass?.id) ?? null,
    items: counts[templates.items.indexOf(template)] ?? 0,
  }));
};

export const TypeList = (): ReactElement => {
  const t = useItemTypesText();
  const { number } = useTranslator();
  const text = useTermLabels();
  return (
    <div className="type-list">
      <p className="type-lede">{t('list.lede')}</p>
      <p>
        <a className="button-link button-link--primary" href="#/resource-templates/new">
          {t('list.new')}
        </a>
      </p>
      <ListPage<TypeRow>
        resourceLabel={t('list.resource')}
        emptyMessage={t('itemTypes.listEmpty')}
        load={load}
        rowKey={(row) => String(row.template.id)}
        columns={[
          {
            header: t('list.type'),
            cell: (row) => (
              <a href={`#/resource-templates/${String(row.template.id)}`}>{row.template.label}</a>
            ),
          },
          {
            header: t('list.fields'),
            cell: (row) => number(row.template.properties.length),
            numeric: true,
          },
          { header: t('list.items'), cell: (row) => number(row.items), numeric: true },
          {
            header: t('list.class'),
            cell: (row) =>
              row.resourceClass === null ? (
                t('class.noneShort')
              ) : (
                <>
                  {plainName(row.resourceClass, text)}{' '}
                  <code className="property-field-term">{row.resourceClass.term}</code>
                </>
              ),
          },
        ]}
      />
    </div>
  );
};
