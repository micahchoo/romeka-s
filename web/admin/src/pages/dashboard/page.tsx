import type { ReactElement } from 'react';
import { api } from '../../api/client';
import type { Site } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';

interface DashboardView {
  readonly itemCount: number;
  readonly itemSetCount: number;
  readonly vocabularyCount: number;
  readonly resourceTemplateCount: number;
  readonly sites: readonly Site[];
}

/** `Admin/IndexController::browseAction`: resource counts plus the site list. */
const load = async (): Promise<DashboardView> => {
  const [items, itemSets, vocabularies, templates, sites] = await Promise.all([
    api.listItems({ perPage: 1 }),
    api.listItemSets({ perPage: 1 }),
    api.listVocabularies({ perPage: 1 }),
    api.listResourceTemplates({ perPage: 1 }),
    api.listSites({ perPage: 25, sortBy: 'id', sortOrder: 'asc' }),
  ]);
  return {
    itemCount: items.total,
    itemSetCount: itemSets.total,
    vocabularyCount: vocabularies.total,
    resourceTemplateCount: templates.total,
    sites: sites.items,
  };
};

export const DashboardPage = (): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(load, []);

  if (state.status === 'loading') {
    return <p role="status">{t('loading.dashboard')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }

  const view = state.data;
  return (
    <section>
      <h2>{t('dashboard.manageResources')}</h2>
      <ul>
        <li>
          <a href="#/items">{t('dashboard.count', { resource: t('page.items'), count: view.itemCount })}</a>
        </li>
        <li>
          <a href="#/item-sets">{t('dashboard.count', { resource: t('page.itemSets'), count: view.itemSetCount })}</a>
        </li>
        <li>
          <a href="#/vocabularies">{t('dashboard.count', { resource: t('page.vocabularies'), count: view.vocabularyCount })}</a>
        </li>
        <li>
          <a href="#/resource-templates">{t('dashboard.count', { resource: t('page.resourceTemplates'), count: view.resourceTemplateCount })}</a>
        </li>
      </ul>
      <h2>{t('dashboard.manageSites')}</h2>
      {view.sites.length === 0 ? (
        <p>{t('dashboard.noSites')}</p>
      ) : (
        <ul>
          {view.sites.map((site) => (
            <li key={site.id}>
              {site.title}
              <span className="site-slug">{` (${site.slug})`}</span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
};
