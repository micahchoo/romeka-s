/**
 * The page editor: one draft, the Outline, the live preview and the inspector.
 * The shell owns the reducer, the selection and the save; the Outline, the
 * preview and the field controls are lazy chunks that receive the draft and
 * dispatch actions back.
 */
import type { ReactElement } from 'react';
import { Suspense, lazy, useEffect, useReducer, useRef, useState } from 'react';
import { defaultBaseUrl } from '../../../../api/client';
import type { JsonValue, PageId, Site, SiteId, SitePage } from '../../../../api/types';
import { SaveStatus } from '../../../../app/fields';
import { useTranslator } from '../../../../i18n';
import { BLOCK_REGISTRY } from '../../blockRegistry';
import type { TemplateChoice } from '../../templates';
import type { EditorOptions } from '../options';
import { emptyDraft, nextBlockKey, validate } from '../draft';
import { isDirty } from '../history';
import { draftReducer } from '../reducer';
import { fromApi } from '../wire';
import type { DraftAction, PageDraft } from '../draft';
import type { ViewedVersion } from './HistoryPanel';
import { layoutLabel } from '../canvas/summary';
import { Inspector } from './Inspector';
import { KeysPopover } from './KeysPopover';
import { PageSettings } from './PageSettings';
import { blockOf, selectedKey } from './selection';
import type { Selection, SelectionSource } from './selection';
import type { SiteWideEdit } from './SiteRegionInspector';
import { REGION_LABEL } from '../regions';
import { commandFor } from './shortcuts';
import type { EditorCommand } from './shortcuts';
import { usePreview } from './usePreview';
import { usePageSave } from './usePageSave';
import { useUnsavedGuard } from './useUnsavedGuard';
import { counted } from '../messages';
import { common } from '../../../../i18n/namespaces/common';
import { sites } from '../../../../i18n/namespaces/sites';
import { EDITOR_NAMESPACES } from '../messages';
import './shell.css';

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

/** The history list and the version view share one chunk, loaded on the first
 * press of History. */
const HistoryPanel = lazy(() =>
  import('./HistoryPanel').then((module) => ({ default: module.HistoryPanel })),
);
const VersionView = lazy(() =>
  import('./HistoryPanel').then((module) => ({ default: module.VersionView })),
);

/** The site region inspector carries the Theme page's controls, so it is its own
 * chunk, loaded on the first region selection. */
const SiteRegionInspector = lazy(() =>
  import('./SiteRegionInspector').then((module) => ({ default: module.SiteRegionInspector })),
);

/** The refused-save dialog is its own chunk, loaded only when a save is refused. */
const ConflictDialog = lazy(() =>
  import('./ConflictDialog').then((module) => ({ default: module.ConflictDialog })),
);

/** The history panel is closed, open on its list, or open on one version. */
type HistoryView =
  | { readonly kind: 'closed' }
  | { readonly kind: 'open'; readonly viewing: ViewedVersion | null };


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

/** Whether `target` is a card in the Outline, or a control inside or around one. */
const onCardOrControl = (target: EventTarget | null): boolean =>
  target instanceof Element &&
  target.closest('.canvas-card, button, a, input, select, textarea, [role="dialog"]') !== null;

export const PageEditor = ({
  siteId,
  pageId,
  page,
  site,
  options,
}: PageEditorProps): ReactElement => {
  const { t } = useTranslator(common, sites, ...EDITOR_NAMESPACES);
  const [draft, dispatch] = useReducer(draftReducer, page, startDraft);
  const [siteSettings, setSiteSettings] = useState<JsonValue | null>(site.siteSettings);
  const [choosing, setChoosing] = useState(page === null);
  const [savingTemplate, setSavingTemplate] = useState(false);
  const [selection, setSelection] = useState<Selection | null>(null);
  const [outlineOpen, setOutlineOpen] = useState(storedOutlineOpen);
  /** The label of the block the last edit removed, while Undo would restore it. */
  const [removed, setRemoved] = useState<string | null>(null);
  const [history, setHistory] = useState<HistoryView>({ kind: 'closed' });
  /** The site's theme settings as last loaded or saved, and the unsaved site-wide change over them. */
  const [storedTheme, setStoredTheme] = useState<JsonValue | null>(site.themeSettings);
  const [siteWide, setSiteWide] = useState<SiteWideEdit | null>(null);
  const preview = usePreview(draft, site.slug, siteId, pageId, siteWide?.settings ?? null);
  const inspector = useRef<HTMLDivElement | null>(null);
  const canvas = useRef<HTMLDivElement | null>(null);
  /** Whether the last press in the Outline began on a card or a control. Selecting a card on that
   * press can move the layout, so its release lands elsewhere and the click fires on a common
   * parent; that click is not a click on the page. */
  const pressedOnCard = useRef(false);
  const focusAfter = useRef<'inspector' | 'canvas' | null>(null);

  const issues = validate(draft);
  const valid = issues.length === 0;
  const dirty = isDirty(draft);
  const siteWideDirty = siteWide?.dirty === true;
  const guard = useUnsavedGuard(
    dirty || siteWideDirty,
    t(dirty ? 'editor.leaveConfirm' : 'region.leaveConfirm'),
  );
  const saving = usePageSave({
    page,
    siteId,
    draft,
    valid,
    dispatch,
    onCreated: (created) => {
      guard.allowLeave();
      window.location.hash = `#/sites/${String(siteId)}/pages/${String(created.id)}`;
    },
    notSaved: t('history.notSaved'),
  });
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
   * it adds is revealed in the other one. A removal offers its own Undo until
   * the next edit, after which Undo would reverse that edit instead. */
  const action = (next: DraftAction, from: SelectionSource = 'outline'): void => {
    const gone =
      next.type === 'removeBlock' ? draft.blocks.find((block) => block.key === next.blockKey) : undefined;
    setRemoved(gone === undefined ? null : layoutLabel(draft.registry, gone, t));
    if (next.type === 'addBlock') {
      setSelection({ kind: 'block', key: nextBlockKey(draft), from });
      focusAfter.current = 'inspector';
    } else if (next.type === 'insertBlocks' && next.blocks.length > 0) {
      setSelection({ kind: 'block', key: nextBlockKey(draft), from });
      focusAfter.current = 'inspector';
    } else if (next.type === 'removeBlock') {
      if (selectedKey(selection) === next.blockKey) {
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

  const restoreVersion = (version: ViewedVersion): void => {
    setHistory({ kind: 'open', viewing: null });
    dispatch({ type: 'loadPage', page: version.page, saved: false });
  };

  const deselect = (): void => {
    setSelection(null);
  };

  const commands: Readonly<Record<EditorCommand, () => void>> = {
    save: saving.save,
    undo: () => {
      action({ type: 'undo' });
    },
    redo: () => {
      action({ type: 'redo' });
    },
    deselect,
  };
  const commandsRef = useRef(commands);
  commandsRef.current = commands;
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      const command = commandFor(event);
      if (command !== null) {
        event.preventDefault();
        commandsRef.current[command]();
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
    setSelection({ kind: 'block', key, from: 'issue' });
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
          onClick={saving.save}
          disabled={!valid || saving.state.status === 'saving'}
        >
          {page === null ? t('page.add') : t('action.save')}
        </button>
        <button type="button" onClick={commands.undo} disabled={draft.past.length === 0}>
          {t('canvas.undo')}
        </button>
        <button type="button" onClick={commands.redo} disabled={draft.future.length === 0}>
          {t('canvas.redo')}
        </button>
        {dirty ? (
          <span className="editor-dirty" role="status">
            <span className="editor-dirty-dot" aria-hidden="true" />
            {t('editor.dirty')}
          </span>
        ) : null}
        {siteWide === null || !siteWideDirty || selection?.kind === 'region' ? null : (
          <button
            type="button"
            className="editor-site-wide-dirty"
            onClick={() => {
              setSelection({ kind: 'region', region: siteWide.region, from: 'inspector' });
              focusAfter.current = 'inspector';
            }}
          >
            <span className="editor-dirty-dot" aria-hidden="true" />
            {t('region.unsavedOpen', { region: t(REGION_LABEL[siteWide.region]) })}
          </button>
        )}
        {issues.length === 0 ? null : (
          <button type="button" className="editor-issues" onClick={jumpToIssue}>
            {counted(t, 'toolbarIssues', issues.length)}
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
        {pageId === null ? null : (
          <button
            type="button"
            className="editor-history-toggle"
            aria-pressed={history.kind === 'open'}
            onClick={() => {
              setSelection(null);
              setHistory(history.kind === 'open' ? { kind: 'closed' } : { kind: 'open', viewing: null });
            }}
          >
            {t('history.button')}
          </button>
        )}
        <KeysPopover />
        {preview.status === 'unavailable' ? (
          <span className="field-note">{t('editor.previewUnavailable')}</span>
        ) : null}
        <SaveStatus state={saving.state} />
      </div>
      <div className="editor-main">
        {outlineShown ? (
          <div
            className="editor-canvas"
            role="region"
            aria-label={t('editor.outline')}
            tabIndex={-1}
            ref={canvas}
            onPointerDown={(event) => {
              pressedOnCard.current = onCardOrControl(event.target);
            }}
            onClick={(event) => {
              // A click that starts and ends on no card and no control is a click on the page itself.
              if (!pressedOnCard.current && !onCardOrControl(event.target)) {
                deselect();
              }
            }}
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
                siteId={siteId}
                selection={selection}
                onSelect={(key) => {
                  setSelection(key === null ? null : { kind: 'block', key, from: 'outline' });
                }}
                dispatch={action}
              />
            </Suspense>
          </div>
        ) : null}
        {history.kind === 'open' && history.viewing !== null && pageId !== null ? (
          <Suspense fallback={<p role="status">{t('editor.loading')}</p>}>
            <VersionView
              version={history.viewing}
              siteSlug={site.slug}
              siteId={siteId}
              pageId={pageId}
              onRestore={() => {
                if (history.viewing !== null) {
                  restoreVersion(history.viewing);
                }
              }}
              onBack={() => {
                setHistory({ kind: 'open', viewing: null });
              }}
            />
          </Suspense>
        ) : preview.status === 'unavailable' ? null : (
          <Suspense fallback={<p role="status">{t('editor.loading')}</p>}>
            <LivePanel
              preview={preview}
              draft={draft}
              selection={selection}
              siteSettings={siteSettings}
              onSelect={(target) => {
                setSelection(target === null ? null : { ...target, from: 'preview' });
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
            {selectedBlock !== null ? (
              <Inspector
                key={selectedBlock.key}
                draft={draft}
                block={selectedBlock}
                dispatch={action}
                onDeselect={() => {
                  deselect();
                  focusAfter.current = 'inspector';
                }}
                onSelect={(key) => {
                  setSelection({ kind: 'block', key, from: 'inspector' });
                  focusAfter.current = 'inspector';
                }}
                issues={issues}
                siteId={siteId}
                options={options}
              />
            ) : selection?.kind === 'region' ? (
              <SiteRegionInspector
                region={selection.region}
                siteId={siteId}
                stored={storedTheme}
                edit={siteWide}
                onEdit={setSiteWide}
                onSaved={(settings) => {
                  setStoredTheme(settings);
                  setSiteWide(null);
                }}
                onPage={() => {
                  deselect();
                  focusAfter.current = 'inspector';
                }}
              />
            ) : history.kind === 'open' && pageId !== null ? (
              <HistoryPanel
                pageId={pageId}
                registry={draft.registry}
                savedAt={saving.loadedModified}
                viewing={history.viewing?.summary.version ?? null}
                onView={(version) => {
                  setHistory({ kind: 'open', viewing: version });
                }}
                onClose={() => {
                  setHistory({ kind: 'closed' });
                }}
              />
            ) : (
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
                onSelectRegion={(region) => {
                  setSelection({ kind: 'region', region, from: 'inspector' });
                  focusAfter.current = 'inspector';
                }}
              />
            )}
          </Suspense>
        </div>
      </div>
      <div className="editor-toast" role="status">
        {removed === null ? null : (
          <>
            <span>{t('canvas.deleted', { label: removed })}</span>
            <button type="button" onClick={commands.undo}>
              {t('canvas.undo')}
            </button>
          </>
        )}
      </div>
      <Suspense fallback={<DialogFallback label={t('loading.page')} />}>
        {saving.conflict === null ? null : (
          <ConflictDialog
            modified={saving.conflict.modified}
            author={saving.conflict.author}
            busy={saving.resolving}
            onShowTheirs={saving.showTheirs}
            onKeepMine={saving.keepMine}
            onClose={saving.dismiss}
          />
        )}
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
