/**
 * The site hub (`#/sites/<id>`): the site's pages first, then a short account
 * of how the site is set up. The frame around it names the site and links to
 * the public site and to the other tabs.
 *
 * Each page row says what a site editor asks of a page: is it public, is it
 * the homepage, is it in the menu. The two row writes are partial updates of
 * one member each, as upstream's Pages screen makes them.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import { api } from '../../api/client';
import { encodeSiteHomepage, encodeSiteNavigation } from '../../api/write';
import type { PageId, Site, SiteId } from '../../api/types';
import { ErrorNotice } from '../../app/errors';
import { SaveStatus } from '../../app/fields';
import { useSave } from '../../app/save';
import { TableScroll } from '../../app/table-scroll';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { firstLinkedPage, linkedPages, pageLinkJson } from './navigationTree';
import { publicUrl, siteHref } from './SiteFrame';
import { common } from '../../i18n/namespaces/common';
import { sites } from '../../i18n/namespaces/sites';

interface HubPage {
  readonly id: PageId;
  /** `null` when the page list did not return this page. */
  readonly title: string | null;
  readonly slug: string | null;
  readonly isPublic: boolean | null;
}

interface Loaded {
  readonly site: Site;
  readonly pages: readonly HubPage[];
}

/** The pages in the site's own order (`o:page`), with what the page list says of each. */
const load = async (siteId: SiteId): Promise<Loaded> => {
  const [site, list] = await Promise.all([
    api.getSite(siteId),
    api.listSitePages({ siteId, perPage: 200 }),
  ]);
  const pages = site.pages.map((ref): HubPage => {
    const page = list.items.find((entry) => entry.id === ref.id);
    return page === undefined
      ? { id: ref.id, title: null, slug: null, isPublic: null }
      : { id: page.id, title: page.title, slug: page.slug, isPublic: page.isPublic };
  });
  return { site, pages };
};

/** The page `/s/<slug>` opens: the homepage, else the first page in the menu. */
const homepageOf = (site: Site): PageId | null =>
  site.homepage?.id ?? firstLinkedPage(site.navigation);

const PageRow = ({
  site,
  page,
  isHomepage,
  isExplicitHomepage,
  inNavigation,
  busy,
  onSetHomepage,
  onClearHomepage,
  onAddToNavigation,
}: {
  readonly site: Site;
  readonly page: HubPage;
  readonly isHomepage: boolean;
  /** The site names this page as its homepage, rather than following the menu. */
  readonly isExplicitHomepage: boolean;
  readonly inNavigation: boolean;
  readonly busy: boolean;
  readonly onSetHomepage: () => void;
  readonly onClearHomepage: () => void;
  readonly onAddToNavigation: () => void;
}): ReactElement => {
  const { t } = useTranslator(common, sites);
  const editHref = `#/sites/${String(site.id)}/pages/${String(page.id)}`;
  const title = page.title ?? t('common.pageFallback', { id: page.id });
  return (
    <tr>
      <th scope="row">
        <a href={editHref}>{title}</a>
        {isHomepage ? <span className="badge badge-accent">{t('siteHub.homepage')}</span> : null}
      </th>
      <td>
        <span className="site-slug">{page.slug ?? ''}</span>
      </td>
      <td>
        {page.isPublic === null ? (
          t('common.unknown')
        ) : (
          <span className={page.isPublic ? 'badge' : 'badge badge-muted'}>
            {page.isPublic ? t('common.public') : t('common.private')}
          </span>
        )}
      </td>
      <td>
        {inNavigation ? (
          t('siteHub.inNavigation')
        ) : (
          <button type="button" className="link-button" disabled={busy} onClick={onAddToNavigation}>
            {t('siteHub.addToNavigation')}
          </button>
        )}
      </td>
      <td>
        <span className="row-actions">
          <a href={editHref}>{t('siteHub.edit')}</a>
          {page.slug === null ? null : (
            <a href={publicUrl(site.slug, page.slug)} target="_blank" rel="noreferrer">
              {t('siteHub.view')}
            </a>
          )}
          {isHomepage ? null : (
            <button type="button" className="link-button" disabled={busy} onClick={onSetHomepage}>
              {t('siteHub.setHomepage')}
            </button>
          )}
          {isExplicitHomepage ? (
            <button type="button" className="link-button" disabled={busy} onClick={onClearHomepage}>
              {t('siteHub.clearHomepage')}
            </button>
          ) : null}
        </span>
      </td>
    </tr>
  );
};

const PagesSection = ({ loaded }: { readonly loaded: Loaded }): ReactElement => {
  const { t } = useTranslator(common, sites);
  const [site, setSite] = useState(loaded.site);
  const { state: saveState, save } = useSave();
  const busy = saveState.status === 'saving';
  const homepage = homepageOf(site);
  const inNavigation = linkedPages(site.navigation);

  const patch = (body: Readonly<Record<string, unknown>>): void => {
    save(async () => {
      setSite(await api.patchSite(site.id, body));
    });
  };

  return (
    <section className="hub-section" aria-labelledby="hub-pages">
      <div className="hub-section-head">
        <h3 id="hub-pages">{t('sites.pages')}</h3>
        <a className="button-link" href={`#/sites/${String(site.id)}/pages/new`}>
          {t('sites.addPage')}
        </a>
      </div>
      {loaded.pages.length === 0 ? (
        <p>{t('sites.noPages')}</p>
      ) : (
        <TableScroll label={t('sites.pages')}>
          <table>
            <thead>
              <tr>
                <th scope="col">{t('column.title')}</th>
                <th scope="col">{t('column.slug')}</th>
                <th scope="col">{t('siteHub.visibility')}</th>
                <th scope="col">{t('page.navigation')}</th>
                <th scope="col">{t('sites.actions')}</th>
              </tr>
            </thead>
            <tbody>
              {loaded.pages.map((page) => (
                <PageRow
                  key={String(page.id)}
                  site={site}
                  page={page}
                  isHomepage={homepage === page.id}
                  isExplicitHomepage={site.homepage?.id === page.id}
                  inNavigation={inNavigation.has(page.id)}
                  busy={busy}
                  onSetHomepage={() => {
                    patch(encodeSiteHomepage(page.id));
                  }}
                  onClearHomepage={() => {
                    patch(encodeSiteHomepage(null));
                  }}
                  onAddToNavigation={() => {
                    patch(encodeSiteNavigation([...site.navigation, pageLinkJson(page.id)]));
                  }}
                />
              ))}
            </tbody>
          </table>
        </TableScroll>
      )}
      <SaveStatus state={saveState} />
    </section>
  );
};

const AboutSection = ({ site }: { readonly site: Site }): ReactElement => {
  const { t } = useTranslator(common, sites);
  return (
    <section className="hub-section" aria-labelledby="hub-about">
      <div className="hub-section-head">
        <h3 id="hub-about">{t('siteHub.about')}</h3>
        <a href={siteHref(site.id, 'settings')}>{t('siteHub.editSettings')}</a>
      </div>
      <dl>
        <dt>{t('siteHub.address')}</dt>
        <dd>
          <a href={publicUrl(site.slug)} target="_blank" rel="noreferrer">
            /s/{site.slug}
          </a>
        </dd>
        <dt>{t('siteHub.visibility')}</dt>
        <dd>{site.isPublic ? t('common.public') : t('common.private')}</dd>
        <dt>{t('sites.theme')}</dt>
        <dd>{site.theme === 'default' ? t('sites.defaultThemeLabel') : site.theme}</dd>
        {site.summary === null ? null : (
          <>
            <dt>{t('sites.summary')}</dt>
            <dd>{site.summary}</dd>
          </>
        )}
      </dl>
    </section>
  );
};

export const SiteDetail = ({ siteId }: { readonly siteId: SiteId }): ReactElement => {
  const { t } = useTranslator(common, sites);
  const state = useAsync(() => load(siteId), [siteId]);
  if (state.status === 'loading') {
    return <p role="status">{t('loading.site')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return (
    <div className="site-hub">
      <PagesSection loaded={state.data} />
      <AboutSection site={state.data.site} />
    </div>
  );
};
