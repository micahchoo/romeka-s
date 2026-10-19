/**
 * The page editor: one draft, the Outline, the live preview and the inspector.
 * The shell owns the reducer, the selection and the save; the Outline, the
 * preview and the field controls are lazy chunks that receive the draft and
 * dispatch actions back.
 */
import type { ReactElement } from 'react';
import { Suspense, lazy, useEffect, useReducer, useRef, useState } from 'react';
import { api, defaultBaseUrl } from '../../../../api/client';
import type { JsonValue, PageId, Site, SiteId, SitePage } from '../../../../api/types';
import { SaveStatus } from '../../../../app/fields';
import { useSave } from '../../../../app/save';
import { useTranslator } from '../../../../i18n';
import { BLOCK_REGISTRY } from '../../blockRegistry';
import type { TemplateChoice } from '../../templates';
import type { EditorOptions } from '../options';
import { emptyDraft, nextBlockKey, validate } from '../draft';
import { isDirty } from '../history';
import { draftReducer } from '../reducer';
import { fromApi, toPageWrite } from '../wire';
import type { DraftAction, PageDraft } from '../draft';
import { Inspector } from './Inspector';
import { PageSettings } from './PageSettings';
import { blockOf } from './selection';
import type { Selection, SelectionSource } from './selection';
import { usePreview } from './usePreview';
import { useUnsavedGuard } from './useUnsavedGuard';
import { common } from '../../../../i18n/namespaces/common';
import { sites } from '../../../../i18n/namespaces/sites';
import { editor } from '../../../../i18n/namespaces/editor';

const PageCanvas = lazy(() =>
  import('../canvas').then((module) => ({ default: module.PageCanvas })),
);

/** The live preview is its own chunk, fetched on the first render beside the Outline. */
const LivePanel = lazy(() =>
  import('./LivePanel').then((module) => ({ default: module.LivePanel })),
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

/** Whether the Outline is open is a per-viewer convenience, so it lives in this
 * browser only and defaults to open when storage is unavailable. */
const OUTLINE_KEY = 'romeka.editor.outline';

const storedOutlineOpen = (): boolean => {
  try {
    return window.localStorage.getItem(OUTLINE_KEY) !== 'closed';
  } catch {
    return true;
  }
};

const storeOutlineOpen = (open: boolean): void => {
  try {
    window.localStorage.setItem(OUTLINE_KEY, open ? 'open' : 'closed');
  } catch {
    // A viewer without storage gets the default again next time.
  }
};

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
  const { t } = useTranslator(common, sites, editor);
  const [draft, dispatch] = useReducer(draftReducer, page, startDraft);
  const [siteSettings, setSiteSettings] = useState<JsonValue | null>(site.siteSettings);
  const [choosing, setChoosing] = useState(page === null);
  const [savingTemplate, setSavingTemplate] = useState(false);
  const [selection, setSelection] = useState<Selection | null>(null);
  const [outlineOpen, setOutlineOpen] = useState(storedOutlineOpen);
  const preview = usePreview(draft, site.slug, siteId, pageId);
  const { state: saveState, save } = useSave();
  const inspector = useRef<HTMLDivElement | null>(null);
  const canvas = useRef<HTMLDivElement | null>(null);
  const focusAfter = useRef<'inspector' | 'canvas' | null>(null);

  const issues = validate(draft);
  const valid = issues.length === 0;
  const dirty = isDirty(draft);
  const guard = useUnsavedGuard(dirty, t('editor.leaveConfirm'));
  const selectedBlock = blockOf(draft, selection);
  // Without a working preview the Outline is the only surface, so it stays open.
  const outlineForced = preview.status === 'unavailable' || preview.status === 'error';
  const outlineShown = outlineOpen || outlineForced;
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

  /** Every edit enters here; `from` names the surface it came from, so a block
   * it adds is revealed in the other one. */
  const action = (next: DraftAction, from: SelectionSource = 'outline'): void => {
    if (next.type === 'addBlock') {
      setSelection({ key: nextBlockKey(draft), from });
      focusAfter.current = 'inspector';
    } else if (next.type === 'insertBlocks' && next.blocks.length > 0) {
      setSelection({ key: nextBlockKey(draft), from });
      focusAfter.current = 'inspector';
    } else if (next.type === 'removeBlock') {
      if (selection?.key === next.blockKey) {
        setSelection(null);
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
    setSelection({ key, from: 'issue' });
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
    <div
      className={[
        'editor',
        outlineShown ? 'editor--outline' : '',
        preview.status === 'unavailable' ? 'editor--no-preview' : '',
      ].join(' ')}
    >
      <div className="editor-toolbar" role="region" aria-label={t('editor.toolbar')}>
        <button
          type="button"
          className="editor-save"
          onClick={runSave}
          disabled={!valid || saveState.status === 'saving'}
        >
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
        <button
          type="button"
          className="editor-outline-toggle"
          aria-pressed={outlineShown}
          disabled={outlineForced}
          title={t('editor.outlineToggle')}
          onClick={() => {
            setOutlineOpen(!outlineOpen);
            storeOutlineOpen(!outlineOpen);
          }}
        >
          {t('editor.outline')}
        </button>
        {preview.status === 'unavailable' ? (
          <span className="field-note">{t('editor.previewUnavailable')}</span>
        ) : null}
        <SaveStatus state={saveState} />
      </div>
      <div className="editor-main">
        {outlineShown ? (
          <div
            className="editor-canvas"
            role="region"
            aria-label={t('editor.outline')}
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
                selection={selection}
                onSelect={(key) => {
                  setSelection(key === null ? null : { key, from: 'outline' });
                }}
                dispatch={action}
              />
            </Suspense>
          </div>
        ) : null}
        {preview.status === 'unavailable' ? null : (
          <Suspense fallback={<p role="status">{t('editor.loading')}</p>}>
            <LivePanel
              preview={preview}
              draft={draft}
              selection={selection}
              siteSettings={siteSettings}
              onSelect={(key) => {
                setSelection({ key, from: 'preview' });
              }}
              dispatch={(next) => {
                action(next, 'preview');
              }}
            />
          </Suspense>
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
                siteId={siteId}
                options={options}
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
