/**
 * The page editor: one draft, a canvas, an inspector and a live preview. The
 * shell owns the reducer and the save; the canvas and the field controls are
 * lazy chunks that receive the draft and dispatch actions back.
 */
import type { ReactElement } from 'react';
import { Suspense, lazy, useEffect, useReducer, useRef, useState } from 'react';
import { api, defaultBaseUrl } from '../../../../api/client';
import type { JsonValue, PageId, Site, SiteId, SitePage } from '../../../../api/types';
import { SaveStatus } from '../../../../app/fields';
import { useSave } from '../../../../app/save';
import { toError } from '../../../../app/useAsync';
import { useTranslator } from '../../../../i18n';
import { BLOCK_REGISTRY } from '../../blockRegistry';
import type { TemplateChoice } from '../../templates';
import type { EditorOptions } from '../options';
import { previewBody, requestPreview } from '../preview';
import {
  draftReducer,
  emptyDraft,
  fromApi,
  isDirty,
  nextBlockKey,
  toPageWrite,
  validate,
} from '../draft';
import type { BlockKey, DraftAction, PageDraft } from '../draft';
import { Inspector } from './Inspector';
import { PageSettings } from './PageSettings';
import { PreviewPanel } from './PreviewPanel';
import type { PreviewStatus } from './PreviewPanel';
import { useUnsavedGuard } from './useUnsavedGuard';

const PageCanvas = lazy(() =>
  import('../canvas').then((module) => ({ default: module.PageCanvas })),
);

/** The create screen's chooser is its own chunk, so the editor shell does not
 * carry the built-in template registry until a page is created. */
const TemplateChooser = lazy(() =>
  import('./TemplateChooser').then((module) => ({ default: module.TemplateChooser })),
);

/** The save-as-template dialog is its own chunk, loaded when the user opens it. */
const SaveTemplateDialog = lazy(() =>
  import('./SaveTemplateDialog').then((module) => ({ default: module.SaveTemplateDialog })),
);

/** The box the lazy dialogs load into. It is the dialog's own box, so showing it
 * while the chunk loads moves nothing on the page. */
const DialogFallback = ({ label }: { readonly label: string }): ReactElement => (
  <div className="template-dialog-backdrop">
    <div className="template-dialog template-dialog-loading" role="status" aria-label={label} />
  </div>
);

/** How long the shell waits after the last change before it asks for a render. */
const PREVIEW_DELAY_MS = 400;

type PreviewMode = 'none' | 'split' | 'full';

export interface PageEditorProps {
  readonly siteId: SiteId;
  readonly pageId: PageId | null;
  readonly page: SitePage | null;
  readonly site: Site;
  readonly options: EditorOptions;
}

const startDraft = (page: SitePage | null): PageDraft =>
  page === null ? emptyDraft() : fromApi(page, BLOCK_REGISTRY);

export const PageEditor = ({
  siteId,
  pageId,
  page,
  site,
  options,
}: PageEditorProps): ReactElement => {
  const { t } = useTranslator();
  const [draft, dispatch] = useReducer(draftReducer, page, startDraft);
  const [siteSettings, setSiteSettings] = useState<JsonValue | null>(site.siteSettings);
  const [choosing, setChoosing] = useState(page === null);
  const [savingTemplate, setSavingTemplate] = useState(false);
  const [selected, setSelected] = useState<BlockKey | null>(null);
  const [preview, setPreview] = useState<PreviewMode>('none');
  const [previewHtml, setPreviewHtml] = useState<string | null>(null);
  const [previewStatus, setPreviewStatus] = useState<PreviewStatus>('idle');
  const [previewMessage, setPreviewMessage] = useState<string | null>(null);
  const { state: saveState, save } = useSave();
  const inspector = useRef<HTMLDivElement | null>(null);
  const canvas = useRef<HTMLDivElement | null>(null);
  const focusAfter = useRef<'inspector' | 'canvas' | null>(null);

  const issues = validate(draft);
  const valid = issues.length === 0;
  const dirty = isDirty(draft);
  const guard = useUnsavedGuard(dirty, t('editor.leaveConfirm'));
  const selectedBlock = draft.blocks.find((block) => block.key === selected) ?? null;
  const publicUrl =
    draft.slug === ''
      ? `${defaultBaseUrl()}/s/${encodeURIComponent(site.slug)}`
      : `${defaultBaseUrl()}/s/${encodeURIComponent(site.slug)}/page/${encodeURIComponent(draft.slug)}`;

  useEffect(() => {
    const target = focusAfter.current;
    focusAfter.current = null;
    if (target === 'inspector') {
      inspector.current?.focus();
    } else if (target === 'canvas') {
      canvas.current?.focus();
    }
  });

  useEffect(() => {
    if (preview === 'none') {
      return;
    }
    let cancelled = false;
    const timer = setTimeout(() => {
      setPreviewStatus('loading');
      requestPreview(site.slug, previewBody(draft, siteId, pageId)).then(
        (result) => {
          if (cancelled) {
            return;
          }
          if (result.kind === 'html') {
            setPreviewHtml(result.html);
            setPreviewStatus('ready');
            setPreviewMessage(null);
          } else if (result.kind === 'unavailable') {
            setPreviewStatus('unavailable');
            setPreviewMessage(null);
          } else {
            setPreviewStatus('error');
            setPreviewMessage(result.message);
          }
        },
        (error: unknown) => {
          if (!cancelled) {
            setPreviewStatus('error');
            setPreviewMessage(toError(error).message);
          }
        },
      );
    }, PREVIEW_DELAY_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [draft, preview, site.slug, siteId, pageId]);

  const action = (next: DraftAction): void => {
    if (next.type === 'addBlock') {
      setSelected(nextBlockKey(draft));
      focusAfter.current = 'inspector';
    } else if (next.type === 'insertBlocks' && next.blocks.length > 0) {
      setSelected(nextBlockKey(draft));
      focusAfter.current = 'inspector';
    } else if (next.type === 'removeBlock') {
      if (selected === next.blockKey) {
        setSelected(null);
      }
      focusAfter.current = 'canvas';
    }
    dispatch(next);
  };

  const chooseTemplate = (template: TemplateChoice): void => {
    dispatch({ type: 'applyTemplate', template });
    setChoosing(false);
  };

  const runSave = (): void => {
    if (!valid || saveState.status === 'saving') {
      return;
    }
    const write = toPageWrite(draft, siteId);
    save(async () => {
      if (page === null) {
        const created = await api.createSitePage(write);
        guard.allowLeave();
        window.location.hash = `#/sites/${String(siteId)}/pages/${String(created.id)}`;
        return;
      }
      await api.updateSitePage(page.id, write);
      dispatch({ type: 'markSaved' });
    });
  };

  const saveRef = useRef(runSave);
  saveRef.current = runSave;
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 's') {
        event.preventDefault();
        saveRef.current();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
    };
  }, []);

  const jumpToIssue = (): void => {
    const issue = issues[0];
    if (issue === undefined) {
      return;
    }
    if (issue.blockKey === null) {
      document.getElementById('page-title')?.focus();
      return;
    }
    const key = issue.blockKey;
    setSelected(key);
    focusAfter.current = null;
    setTimeout(() => {
      const control = document.getElementById(`block-${key}-${issue.field}`);
      if (control instanceof HTMLElement) {
        control.focus();
      } else {
        inspector.current?.focus();
      }
    }, 0);
  };

  if (choosing) {
    return (
      <Suspense
        fallback={
          <div className="template-chooser template-chooser-loading" role="status" aria-label={t('loading.page')} />
        }
      >
        <TemplateChooser
          registry={BLOCK_REGISTRY}
          siteSettings={siteSettings}
          onChoose={chooseTemplate}
          onBlank={() => {
            setChoosing(false);
          }}
        />
      </Suspense>
    );
  }

  return (
    <div className={`editor editor--preview-${preview}`}>
      <div className="editor-toolbar" role="region" aria-label={t('editor.toolbar')}>
        <button type="button" onClick={runSave} disabled={!valid || saveState.status === 'saving'}>
          {page === null ? t('page.add') : t('action.save')}
        </button>
        {dirty ? (
          <span className="editor-dirty" role="status">
            <span className="editor-dirty-dot" aria-hidden="true" />
            {t('editor.dirty')}
          </span>
        ) : null}
        {issues.length === 0 ? null : (
          <button type="button" className="editor-issues" onClick={jumpToIssue}>
            {issues.length === 1
              ? t('editor.issueOne')
              : t('editor.issueCount', { count: issues.length })}
          </button>
        )}
        {pageId === null ? null : (
          <a href={publicUrl} target="_blank" rel="noreferrer">
            {t('editor.viewPublic')}
          </a>
        )}
        <div className="editor-preview-toggle" role="group" aria-label={t('editor.preview')}>
          {(
            [
              ['none', t('editor.previewNone')],
              ['split', t('editor.previewSplit')],
              ['full', t('editor.previewFull')],
            ] as const
          ).map(([mode, label]) => (
            <button
              key={mode}
              type="button"
              aria-pressed={preview === mode}
              onClick={() => {
                setPreview(mode);
              }}
            >
              {label}
            </button>
          ))}
        </div>
        <SaveStatus state={saveState} />
      </div>
      <div className="editor-main">
        {preview === 'full' ? null : (
          <div
            className="editor-canvas"
            role="region"
            aria-label={t('editor.canvas')}
            tabIndex={-1}
            ref={canvas}
          >
            <Suspense fallback={<p role="status">{t('editor.loading')}</p>}>
              {draft.blocks.length === 0 ? (
                <p className="editor-empty">
                  <button
                    type="button"
                    onClick={() => {
                      action({ type: 'addBlock', layout: 'html', index: 0 });
                    }}
                  >
                    {t('editor.empty')}
                  </button>
                </p>
              ) : null}
              <PageCanvas
                draft={draft}
                siteSettings={siteSettings}
                selectedKey={selected}
                onSelect={setSelected}
                dispatch={action}
              />
            </Suspense>
          </div>
        )}
        {preview === 'none' ? null : (
          <PreviewPanel html={previewHtml} status={previewStatus} message={previewMessage} />
        )}
        <div
          className="editor-inspector"
          role="region"
          aria-label={t('editor.inspector')}
          tabIndex={-1}
          ref={inspector}
        >
          <Suspense fallback={<p role="status">{t('editor.loading')}</p>}>
            {selectedBlock === null ? (
              <PageSettings
                draft={draft}
                dispatch={action}
                issues={issues}
                pageId={pageId}
                onDeleted={() => {
                  guard.allowLeave();
                  window.location.hash = `#/sites/${String(siteId)}`;
                }}
                onOpenSaveTemplate={() => {
                  setSavingTemplate(true);
                }}
              />
            ) : (
              <Inspector
                key={selectedBlock.key}
                draft={draft}
                block={selectedBlock}
                dispatch={action}
                issues={issues}
                siteId={siteId}
                options={options}
              />
            )}
          </Suspense>
        </div>
      </div>
      <Suspense fallback={<DialogFallback label={t('loading.page')} />}>
        {savingTemplate ? (
          <SaveTemplateDialog
            siteId={siteId}
            siteSettings={siteSettings}
            draft={draft}
            onSaved={(settings) => {
              setSiteSettings(settings);
              setSavingTemplate(false);
            }}
            onClose={() => {
              setSavingTemplate(false);
            }}
          />
        ) : null}
      </Suspense>
    </div>
  );
};
