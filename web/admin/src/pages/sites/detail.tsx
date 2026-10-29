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
import { homepageOf, linkedPages, pageLinkJson } from './navigationTree';
import { SiteCheck } from './SiteCheck';
import { AccessNote, publicUrl, siteHref, themeLabel, useSiteAccess } from './SiteFrame';
import type { SiteCapability } from './siteAccess';
import { common } from '../../i18n/namespaces/common';
import { sites } from '../../i18n/namespaces/sites';

interface HubPage {
  readonly id: PageId;
  /** `null` when the page list did not return this page. */
  readonly title: string | null;
  readonly slug: string | null;
  readonly isPublic: boolean | null;
  /** How many of its stored blocks are not written yet, `null` when unknown. */
  readonly unwritten: number | null;
}

interface Loaded {
  readonly site: Site;
  readonly pages: readonly HubPage[];
  /** How many items the site holds. */
  readonly items: number;
}

/** The pages in the site's own order (`o:page`), with what the page list says of each. */
const load = async (siteId: SiteId): Promise<Loaded> => {
  // The block registry is the editor's, so the count's module loads beside
  // the hub rather than inside its chunk.
  const [site, list, items, { storedUnwrittenCount }] = await Promise.all([
    api.getSite(siteId),
    api.listSitePages({ siteId, perPage: 200 }),
    api.listItems({ siteId, perPage: 1 }),
    import('./editor/written'),
  ]);
  const pages = site.pages.map((ref): HubPage => {
    const page = list.items.find((entry) => entry.id === ref.id);
    return page === undefined
      ? { id: ref.id, title: null, slug: null, isPublic: null, unwritten: null }
      : {
          id: page.id,
          title: page.title,
          slug: page.slug,
          isPublic: page.isPublic,
          unwritten: storedUnwrittenCount(page.blocks),
        };
  });
  return { site, pages, items: items.total };
};

const PageRow = ({
  site,
  page,
  isHomepage,
  isExplicitHomepage,
  inNavigation,
  busy,
  access,
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
  readonly access: ReadonlySet<SiteCapability>;
  readonly onSetHomepage: () => void;
  readonly onClearHomepage: () => void;
  readonly onAddToNavigation: () => void;
}): ReactElement => {
  const { t } = useTranslator(common, sites);
  const editHref = `#/sites/${String(site.id)}/pages/${String(page.id)}`;
  const title = page.title ?? t('common.pageFallback', { id: page.id });
  const canEdit = access.has('editPages');
  const canChange = access.has('changeSite');
  return (
    <tr>
      <th scope="row">
        {canEdit ? <a href={editHref}>{title}</a> : title}
        {isHomepage ? <span className="badge badge-accent">{t('siteHub.homepage')}</span> : null}
        {page.unwritten === null || page.unwritten === 0 ? null : (
          <span className="badge badge-muted page-unwritten">
            {page.unwritten === 1
              ? t('siteHub.unwrittenOne')
              : t('siteHub.unwritten', { count: page.unwritten })}
          </span>
        )}
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
        ) : canChange ? (
          <button type="button" className="link-button" disabled={busy} onClick={onAddToNavigation}>
            {t('siteHub.addToNavigation')}
          </button>
        ) : (
          t('siteAccess.notInNavigation')
        )}
      </td>
      <td>
        <span className="row-actions">
          {canEdit ? <a href={editHref}>{t('siteHub.edit')}</a> : null}
          {page.slug === null ? null : (
            <a href={publicUrl(site.slug, page.slug)} target="_blank" rel="noreferrer">
              {t('siteHub.view')}
            </a>
          )}
          {isHomepage || !canChange ? null : (
            <button type="button" className="link-button" disabled={busy} onClick={onSetHomepage}>
              {t('siteHub.setHomepage')}
            </button>
          )}
          {isExplicitHomepage && canChange ? (
            <button type="button" className="link-button" disabled={busy} onClick={onClearHomepage}>
              {t('siteHub.clearHomepage')}
            </button>
          ) : null}
        </span>
      </td>
    </tr>
  );
};

const PagesSection = ({
  loaded,
  access,
}: {
  readonly loaded: Loaded;
  readonly access: ReadonlySet<SiteCapability>;
}): ReactElement => {
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
        {access.has('editPages') ? (
          <a className="button-link" href={`#/sites/${String(site.id)}/pages/new`}>
            {t('sites.addPage')}
          </a>
        ) : null}
      </div>
      {!access.has('editPages') ? (
        <AccessNote>{t('siteAccess.viewer')}</AccessNote>
      ) : !access.has('changeSite') ? (
        <AccessNote>{t('siteAccess.hub')}</AccessNote>
      ) : null}
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
                  access={access}
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

const AboutSection = ({
  site,
  access,
  items,
}: {
  readonly site: Site;
  readonly access: ReadonlySet<SiteCapability>;
  readonly items: number;
}): ReactElement => {
  const { t, number } = useTranslator(common, sites);
  return (
    <section className="hub-section" aria-labelledby="hub-about">
      <div className="hub-section-head">
        <h3 id="hub-about">{t('siteHub.about')}</h3>
        {access.has('changeSite') ? (
          <a href={siteHref(site.id, 'settings')}>{t('siteHub.editSettings')}</a>
        ) : null}
      </div>
      <dl>
        <dt>{t('siteHub.address')}</dt>
        <dd>
          <a href={publicUrl(site.slug)} target="_blank" rel="noreferrer">
            /s/{site.slug}
          </a>
        </dd>
        <dt>{t('siteHub.items')}</dt>
        <dd>
          <a href={siteHref(site.id, 'resources')}>
            {items === 0 ? t('siteHub.noItems') : t('itemPicker.setCount', { count: number(items) })}
          </a>
        </dd>
        <dt>{t('siteHub.visibility')}</dt>
        <dd>{site.isPublic ? t('common.public') : t('common.private')}</dd>
        <dt>{t('sites.theme')}</dt>
        <dd>{themeLabel(site.theme, t)}</dd>
        {site.summary === null ? null : (
          <>
            <dt>{t('sites.summary')}</dt>
            <dd>{site.summary}</dd>
          </>
        )}
      </dl>
      {access.has('editPages') ? <SiteCheck siteId={site.id} settings={access.has('changeSite')} /> : null}
    </section>
  );
};

export const SiteDetail = ({ siteId }: { readonly siteId: SiteId }): ReactElement => {
  const { t } = useTranslator(common, sites);
  const state = useAsync(() => load(siteId), [siteId]);
  const access = useSiteAccess();
  if (state.status === 'loading' || access === null) {
    return <p role="status">{t('loading.site')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return (
    <div className="site-hub">
      <PagesSection loaded={state.data} access={access} />
      <AboutSection site={state.data.site} items={state.data.items} access={access} />
    </div>
  );
};
