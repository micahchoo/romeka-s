/**
 * The page history: the list of saved versions, shown in the inspector column,
 * and the view of one version, shown where the live preview was. A version is
 * rendered through the preview route, so viewing it saves nothing. Both are
 * this one lazy chunk, fetched the first time the History button is pressed.
 */
import type { ReactElement } from 'react';
import { useEffect, useState } from 'react';
import { api } from '../../../../api/client';
import type { PageId, PageVersionSummary, SiteId, SitePage } from '../../../../api/types';
import { ErrorNotice } from '../../../../app/errors';
import { toError, useAsync } from '../../../../app/useAsync';
import { useTranslator } from '../../../../i18n';
import { counted } from '../messages';
import type { BlockRegistry } from '../../blockSchema';
import { sites } from '../../../../i18n/namespaces/sites';
import { authorText, changesText } from './versionChanges';
import { previewRequest, requestPreview } from '../preview';
import { contentFromApi } from '../wire';
import { versionTime } from './versionTime';
import { common } from '../../../../i18n/namespaces/common';
import { EDITOR_NAMESPACES } from '../messages';
import './history.css';

/** A version the user opened: its list entry and the page it stored. */
export interface ViewedVersion {
  readonly summary: PageVersionSummary;
  readonly page: SitePage;
}

export interface HistoryPanelProps {
  readonly pageId: PageId;
  /** Names the changed blocks a version lists. */
  readonly registry: BlockRegistry;
  /** Changes after every save, so the list reloads and shows the new version. */
  readonly savedAt: string | null;
  readonly viewing: number | null;
  readonly onView: (version: ViewedVersion) => void;
  readonly onClose: () => void;
}

export const HistoryPanel = ({
  pageId,
  registry,
  savedAt,
  viewing,
  onView,
  onClose,
}: HistoryPanelProps): ReactElement => {
  const translator = useTranslator(common, sites, ...EDITOR_NAMESPACES);
  const { t } = translator;
  const versions = useAsync(() => api.listPageVersions(pageId), [pageId, savedAt]);
  const [opening, setOpening] = useState<number | null>(null);
  const [openError, setOpenError] = useState<Error | null>(null);

  const open = (summary: PageVersionSummary): void => {
    setOpening(summary.version);
    setOpenError(null);
    api.getPageVersion(pageId, summary.version).then(
      (page) => {
        setOpening(null);
        onView({ summary, page });
      },
      (error: unknown) => {
        setOpening(null);
        setOpenError(toError(error));
      },
    );
  };

  const label = (summary: PageVersionSummary): string =>
    t('history.entry', {
      time: versionTime(summary.created, translator.locale, (time) => t('history.today', { time })),
      name: authorText(summary, t),
      blocks: counted(t, 'blocks', summary.blockCount),
    });

  return (
    <section className="history-panel" aria-label={t('history.title')}>
      <div className="history-head">
        <h2>{t('history.title')}</h2>
        <button type="button" onClick={onClose}>
          {t('history.close')}
        </button>
      </div>
      {versions.status === 'loading' ? <p role="status">{t('history.loading')}</p> : null}
      {versions.status === 'error' ? <ErrorNotice error={versions.error} /> : null}
      {versions.status === 'ready' && versions.data.length === 0 ? (
        <p className="field-note">{t('history.empty')}</p>
      ) : null}
      {versions.status === 'ready' && versions.data.length > 0 ? (
        <ol className="history-list">
          {versions.data.map((summary, index) => (
            <li key={summary.version}>
              <button
                type="button"
                aria-pressed={viewing === summary.version}
                aria-busy={opening === summary.version}
                onClick={() => {
                  open(summary);
                }}
              >
                <span className="history-entry">{label(summary)}</span>
                <span className="history-changes">
                  {changesText(summary, registry, translator.locale, t)}
                </span>
              </button>
              {index === 0 ? <span className="history-live">{t('history.live')}</span> : null}
            </li>
          ))}
        </ol>
      ) : null}
      {openError === null ? null : <ErrorNotice error={openError} />}
    </section>
  );
};

export interface VersionViewProps {
  readonly version: ViewedVersion;
  readonly siteSlug: string;
  readonly siteId: SiteId;
  readonly pageId: PageId;
  readonly onRestore: () => void;
  readonly onBack: () => void;
}

type Render =
  | { readonly status: 'loading' }
  | { readonly status: 'ready'; readonly html: string }
  | { readonly status: 'error'; readonly message: string };

/** One version, rendered by the preview route in place of the live preview. */
export const VersionView = ({
  version,
  siteSlug,
  siteId,
  pageId,
  onRestore,
  onBack,
}: VersionViewProps): ReactElement => {
  const translator = useTranslator(...EDITOR_NAMESPACES);
  const { t } = translator;
  const [render, setRender] = useState<Render>({ status: 'loading' });

  useEffect(() => {
    const controller = new AbortController();
    setRender({ status: 'loading' });
    const { body } = previewRequest(contentFromApi(version.page), siteId, pageId);
    requestPreview(siteSlug, body, controller.signal).then(
      (result) => {
        if (result.kind === 'html') {
          setRender({ status: 'ready', html: result.html });
        } else {
          setRender({
            status: 'error',
            message: result.kind === 'error' ? result.message : t('history.renderFailed'),
          });
        }
      },
      (error: unknown) => {
        if (!controller.signal.aborted) {
          setRender({ status: 'error', message: toError(error).message });
        }
      },
    );
    return () => {
      controller.abort();
    };
  }, [version, siteSlug, siteId, pageId, t]);

  const time = versionTime(version.summary.created, translator.locale, (shown) =>
    t('history.today', { time: shown }),
  );

  return (
    <section className="editor-preview history-view" aria-label={t('history.viewing', { time })}>
      <div className="history-banner">
        <p>{t('history.viewing', { time })}</p>
        <button type="button" className="history-restore" onClick={onRestore}>
          {t('history.restore')}
        </button>
        <button type="button" onClick={onBack}>
          {t('history.back')}
        </button>
      </div>
      <div className="editor-preview-frame">
        {render.status === 'ready' ? (
          <iframe
            className="history-frame"
            sandbox=""
            srcDoc={render.html}
            title={t('history.viewing', { time })}
          />
        ) : (
          <p className="editor-preview-empty" role="status">
            {render.status === 'loading' ? t('history.rendering') : render.message}
          </p>
        )}
      </div>
    </section>
  );
};
