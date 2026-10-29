/**
 * The page editor route: it loads the site, the stored page (or nothing for a
 * create) and the option lists the block fields share, then hands them to one
 * editor. The route module is lazy, so nothing here loads until a page opens.
 */
import type { ReactElement } from 'react';
import { api } from '../../../../api/client';
import type { PageId, Site, SiteId, SitePage } from '../../../../api/types';
import { ErrorNotice } from '../../../../app/errors';
import { useAsync } from '../../../../app/useAsync';
import { useTranslator } from '../../../../i18n';
import { useEditorText } from '../messages';
import { listsAttachedOnly } from '../fields/listReach';
import { loadEditorOptions } from '../options';
import type { EditorOptions } from '../options';
import { PageEditor } from './PageEditor';

interface Loaded {
  readonly site: Site;
  readonly page: SitePage | null;
  readonly options: EditorOptions;
}

const load = async (siteId: SiteId, pageId: PageId | null): Promise<Loaded> => {
  const [site, page] = await Promise.all([
    api.getSite(siteId),
    pageId === null ? Promise.resolve(null) : api.getSitePage(pageId),
  ]);
  const options = await loadEditorOptions(siteId);
  return {
    site,
    page,
    options: { ...options, themeSettings: site.themeSettings, attachedOnly: listsAttachedOnly(site.siteSettings) },
  };
};

export const SitePageEditor = ({
  siteId,
  pageId,
}: {
  readonly siteId: SiteId;
  readonly pageId: PageId | null;
}): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(() => load(siteId, pageId), [siteId, pageId]);
  // The editor's own table suspends the route until it arrives, so no label renders in English.
  useEditorText();

  if (state.status === 'loading') {
    return <p role="status">{t('loading.page')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return (
    <PageEditor
      key={pageId === null ? 'new' : String(pageId)}
      siteId={siteId}
      pageId={pageId}
      site={state.data.site}
      page={state.data.page}
      options={state.data.options}
    />
  );
};
