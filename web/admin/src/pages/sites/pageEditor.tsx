/**
 * The site page editor (`SiteAdmin/PageController::editAction`, with create via
 * `SiteAdmin/IndexController::addPageAction`). It edits the page's title, slug
 * and visibility plus its blocks; saving replaces `o:block` with the list on
 * screen.
 */
import type { ReactElement, SyntheticEvent } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { parseJsonText } from '../../api/parse';
import type { JsonValue, PageId, SiteId, SitePage, SitePageWrite } from '../../api/types';
import { DeleteControl } from '../../app/delete';
import { CheckboxField, SaveStatus, SubmitRow, TextField } from '../../app/fields';
import { ErrorNotice } from '../../app/errors';
import { formatJson } from '../../app/json';
import { useSave } from '../../app/save';
import { toError, useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import {
  BlockEditor,
  blockWriteError,
  blocksFromPage,
  toBlockWrites,
  type BlockDraft,
} from './blocks';

/** A new page starts with the page title block upstream's editor seeds. */
const NEW_PAGE_BLOCKS: readonly BlockDraft[] = [
  { key: 0, layout: 'pageTitle', data: '[]', layoutData: '' },
];

const PageEditor = ({
  siteId,
  page,
}: {
  readonly siteId: SiteId;
  readonly page: SitePage | null;
}): ReactElement => {
  const translator = useTranslator();
  const { t } = translator;
  const [title, setTitle] = useState(page?.title ?? '');
  const [slug, setSlug] = useState(page?.slug ?? '');
  const [isPublic, setIsPublic] = useState(page?.isPublic ?? true);
  const [layout, setLayout] = useState(page?.layout ?? '');
  const [layoutData, setLayoutData] = useState(() => formatJson(page?.layoutData ?? []));
  const [blocks, setBlocks] = useState<readonly BlockDraft[]>(() =>
    page === null ? NEW_PAGE_BLOCKS : blocksFromPage(page.blocks),
  );
  const [formError, setFormError] = useState<string | null>(null);
  const { state: saveState, save } = useSave();

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (title.trim() === '') {
      setFormError(t('page.errorNoTitle'));
      return;
    }
    let pageLayoutData: JsonValue;
    try {
      pageLayoutData = parseJsonText(layoutData);
    } catch (error) {
      setFormError(`${t('page.layoutData')}: ${toError(error).message}`);
      return;
    }
    const parsedBlocks = toBlockWrites(blocks);
    if (parsedBlocks.error !== null) {
      setFormError(blockWriteError(parsedBlocks.error, translator));
      return;
    }
    setFormError(null);
    const trimmedLayout = layout.trim();
    const write: SitePageWrite = {
      site: siteId,
      title: title.trim(),
      slug: slug.trim(),
      isPublic,
      layout: trimmedLayout === '' ? null : trimmedLayout,
      layoutData: pageLayoutData,
      blocks: parsedBlocks.blocks,
    };
    save(async () => {
      if (page === null) {
        const created = await api.createSitePage(write);
        window.location.hash = `#/sites/${String(siteId)}/pages/${String(created.id)}`;
      } else {
        await api.updateSitePage(page.id, write);
      }
    });
  };

  return (
    <div>
      <p>
        <a href={`#/sites/${String(siteId)}`}>{t('page.backToSite')}</a>
      </p>
      <form onSubmit={submit}>
        <fieldset>
          <legend>{t('page.legend')}</legend>
          <TextField id="page-title" label={t('sites.title')} value={title} onChange={setTitle} />
          <TextField id="page-slug" label={t('sites.slug')} value={slug} onChange={setSlug} />
          <CheckboxField
            id="page-public"
            label={t('common.public')}
            checked={isPublic}
            onChange={setIsPublic}
          />
          <TextField id="page-layout" label={t('page.layout')} value={layout} onChange={setLayout} />
          <p>
            <label htmlFor="page-layout-data">{t('page.layoutData')}</label>
            <textarea
              id="page-layout-data"
              rows={3}
              value={layoutData}
              onChange={(event) => {
                setLayoutData(event.target.value);
              }}
            />
          </p>
        </fieldset>
        <BlockEditor blocks={blocks} onChange={setBlocks} />
        {formError === null ? null : <p role="alert">{formError}</p>}
        <SaveStatus state={saveState} />
        <SubmitRow
          label={page === null ? t('page.add') : t('action.save')}
          busy={saveState.status === 'saving'}
        />
      </form>
      {page === null ? null : (
        <DeleteControl
          label={t('page.deleteLabel')}
          rowName={page.title}
          onConfirm={async () => {
            await api.deleteSitePage(page.id);
            window.location.hash = `#/sites/${String(siteId)}`;
          }}
        />
      )}
    </div>
  );
};

/** Loads the stored page, then edits it. A `null` id is a create. */
export const SitePageEditor = ({
  siteId,
  pageId,
}: {
  readonly siteId: SiteId;
  readonly pageId: PageId | null;
}): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(
    async (): Promise<SitePage | null> => (pageId === null ? null : api.getSitePage(pageId)),
    [pageId],
  );

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
      page={state.data}
    />
  );
};
