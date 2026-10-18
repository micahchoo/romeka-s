import type { ReactElement } from 'react';
import { api } from '../../api/client';
import type { PageId, ResourceRef, Site, SiteId, User } from '../../api/types';
import { DeleteControl } from '../../app/delete';
import { ErrorNotice } from '../../app/errors';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import type { MessageKey } from '../../i18n';
import { SiteEditForm, type PageSummary } from './editor';

interface Loaded {
  readonly site: Site;
  readonly users: readonly User[];
  readonly pages: readonly PageSummary[];
}

/**
 * The title of each page in the site's `o:page` list. The API refuses a
 * non-public page to a caller without the site role, so a failed read keeps the
 * page in the list by id instead of dropping it.
 */
const loadPageSummaries = async (
  refs: readonly ResourceRef<PageId>[],
): Promise<readonly PageSummary[]> =>
  Promise.all(
    refs.map(async (ref): Promise<PageSummary> => {
      try {
        const page = await api.getSitePage(ref.id);
        return { id: ref.id, title: page.title, isPublic: page.isPublic };
      } catch {
        return { id: ref.id, title: null, isPublic: null };
      }
    }),
  );

const load = async (siteId: SiteId): Promise<Loaded> => {
  const site = await api.getSite(siteId);
  const [users, pages] = await Promise.all([
    api.listUsers({ perPage: 200, sortBy: 'name' }),
    loadPageSummaries(site.pages),
  ]);
  return { site, users: users.items, pages };
};

const PageList = ({
  siteId,
  pages,
}: {
  readonly siteId: SiteId;
  readonly pages: readonly PageSummary[];
}): ReactElement => {
  const { t } = useTranslator();
  return (
    <section>
      <h2>{t('sites.pages')}</h2>
      <p>
        <a href={`#/sites/${String(siteId)}/pages/new`}>{t('sites.addPage')}</a>
      </p>
      {pages.length === 0 ? (
        <p>{t('sites.noPages')}</p>
      ) : (
        <ul>
          {pages.map((page) => (
            <li key={String(page.id)}>
              <a href={`#/sites/${String(siteId)}/pages/${String(page.id)}`}>
                {page.title ?? t('common.pageFallback', { id: page.id })}
              </a>
              {page.isPublic === false ? <span> {t('sites.pagePrivate')}</span> : null}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
};

/**
 * The other site-admin pages, one per member group the site document carries
 * (`SiteAdmin/IndexController::navigationAction`, `::resourcesAction`,
 * `::usersAction`, `::themeAction`).
 */
const SITE_SECTIONS: readonly { readonly segment: string; readonly labelKey: MessageKey }[] = [
  { segment: 'navigation', labelKey: 'page.navigation' },
  { segment: 'resources', labelKey: 'page.siteResources' },
  { segment: 'users', labelKey: 'page.siteUsers' },
  { segment: 'settings', labelKey: 'page.siteSettings' },
];

const SectionLinks = ({ siteId }: { readonly siteId: SiteId }): ReactElement => {
  const { t } = useTranslator();
  return (
    <nav aria-label={t('sites.sectionsLabel')}>
      <ul>
        {SITE_SECTIONS.map((section) => (
          <li key={section.segment}>
            <a href={`#/sites/${String(siteId)}/${section.segment}`}>{t(section.labelKey)}</a>
          </li>
        ))}
      </ul>
    </nav>
  );
};

/** The site edit form and its page list. */
export const SiteDetail = ({ siteId }: { readonly siteId: SiteId }): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(() => load(siteId), [siteId]);

  if (state.status === 'loading') {
    return <p role="status">{t('loading.site')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  const site = state.data.site;
  return (
    <div>
      <SectionLinks siteId={siteId} />
      <SiteEditForm
        site={site}
        users={state.data.users}
        pages={state.data.pages}
      />
      <PageList siteId={siteId} pages={state.data.pages} />
      <DeleteControl
        label={t('sites.deleteLabel')}
        rowName={site.title}
        onConfirm={async () => {
          await api.deleteSite(site.id);
          window.location.hash = '#/sites';
        }}
      />
    </div>
  );
};
