/**
 * The site Theme page: pick a preset, set a brand colour, fine-tune single
 * tokens, write custom CSS, order the resource page sections, and watch the
 * site's homepage, or a sample item, item set or media page, change beside the
 * controls.
 *
 * The draft (`themeEditor/draft.ts`) is the stored shape the theme reads. A
 * save PATCHes it as the site's `o:theme_settings`; a preview POSTs it to
 * `/s/<slug>/preview`, which renders it and stores nothing. The swatches and
 * the contrast gate resolve the draft in the browser, with the same
 * arithmetic as the theme, so they react on every keystroke; a palette below
 * AA 4.5:1 cannot be saved. Custom CSS is checked by the server's sanitiser,
 * through the preview while the curator types and again on save.
 */
import type { ReactElement } from 'react';
import { Suspense, lazy, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ApiError, api } from '../../api/client';
import { isRecord } from '../../api/parse';
import type { ApiErrors, JsonValue, Site, SiteId, SitePage } from '../../api/types';
import { encodeSiteThemeSettings } from '../../api/write';
import { ErrorNotice } from '../../app/errors';
import { SaveStatus } from '../../app/fields';
import { useSave } from '../../app/save';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { AssetPicker } from './editor/fields/AssetPicker';
import { HtmlEditor } from './editor/fields/HtmlEditor';
import { PreviewPanel } from './editor/shell/PreviewPanel';
import type { PreviewStatus } from './editor/shell/PreviewPanel';
import { requestPreview } from './editor/preview';
import { ResourcePageBlocksField } from './ResourcePageBlocksField';
import { RESOURCE_PAGE_BLOCKS_REGISTRY } from './resourcePageBlocksRegistry';
import { resourcePageBlockDrafts, resourcePageBlocksWire } from './resourcePageBlocksSchema';
import type { ResourcePageBlocksDraft } from './resourcePageBlocksSchema';
import { BrandPanel, ContrastPanel } from './themeEditor/BrandPanel';
import { FineTune } from './themeEditor/FineTune';
import { PresetGallery } from './themeEditor/PresetGallery';
import {
  applyPreset,
  contrastChecks,
  draftFromStored,
  draftProblems,
  draftWire,
  exportTheme,
  importTheme,
  resolve,
  setOverride,
} from './themeEditor/draft';
import type { CssRefusal } from './themeEditor/CustomCssTab';
import type { ContentKey, ThemeDraft } from './themeEditor/draft';
import { THEME_REGISTRY, settingFor } from './themeEditor/registry';
import {
  NONE_KEYS,
  PreviewSwitch,
  resourcePreviewBody,
  useSample,
} from './themeEditor/previewTarget';
import type { PreviewTarget } from './themeEditor/previewTarget';
import { useThemeText } from './themeEditor/messages';
import { common } from '../../i18n/namespaces/common';
import { sites } from '../../i18n/namespaces/sites';
import { SaveBar, useUnsavedTab } from './unsaved';
import { AccessNote, useSiteAccess } from './SiteFrame';
import './themeEditor/theme.css';

const CustomCssTab = lazy(() => import('./themeEditor/CustomCssTab'));

/** The CSS editor's byte ceiling, from the generated schema. */
const CSS_MAX_BYTES = ((): number => {
  const setting = settingFor('custom_css');
  return setting?.kind === 'css' ? setting.maxBytes : 65536;
})();

const storedSettings = (site: Site): Readonly<Record<string, unknown>> => {
  const value = site.themeSettings;
  return isRecord(value) ? value : {};
};

/** The preview request body: the stored page plus the unsaved theme map. */
const previewBody = (
  page: SitePage,
  settings: Readonly<Record<string, JsonValue>>,
): Readonly<Record<string, unknown>> => ({
  'o:id': page.id,
  'o:title': page.title,
  'o:slug': page.slug,
  'o:is_public': page.isPublic,
  'o:layout': page.layout ?? '',
  'o:layout_data': page.layoutData,
  'o:theme_settings': settings,
});

interface PreviewState {
  readonly html: string | null;
  readonly status: PreviewStatus;
  readonly message: string | null;
}

/**
 * The sanitiser's refusals for the theme map: each message in the active
 * locale, with the line its English names (`Custom CSS: Line 3, column 5: ...`).
 * A translation may move the number, so it is read from the server's text.
 */
const cssRefusals = (errors: ApiErrors | null, sourceErrors: ApiErrors | null): readonly CssRefusal[] => {
  const source = sourceErrors?.['o:theme_settings'] ?? [];
  return (errors?.['o:theme_settings'] ?? []).map((message, index) => {
    const line = /Line (\d+), column \d+/.exec(source[index] ?? '')?.[1];
    return { message, line: line === undefined ? null : Number(line) };
  });
};

type Tab = 'design' | 'css';

/** The header and footer content: logo, banner and footer, each with its help line. */
const ContentSection = ({
  draft,
  onChange,
}: {
  readonly draft: ThemeDraft;
  readonly onChange: (key: ContentKey, value: string) => void;
}): ReactElement => {
  const tt = useThemeText();
  const help = (key: ContentKey): ReactElement | null => {
    const setting = settingFor(key);
    return setting === undefined ? null : <p className="field-note theme-help">{tt(setting.helpKey)}</p>;
  };
  return (
    <fieldset className="theme-section">
      <legend>{tt('theme.headerFooter')}</legend>
      <AssetPicker
        id="theme-setting-logo"
        label={tt('themeSetting.logo')}
        value={draft.content.logo}
        onChange={(value) => {
          onChange('logo', value);
        }}
      />
      {help('logo')}
      <AssetPicker
        id="theme-setting-banner"
        label={tt('themeSetting.banner')}
        value={draft.content.banner}
        onChange={(value) => {
          onChange('banner', value);
        }}
      />
      {help('banner')}
      <p>
        <label htmlFor="theme-setting-banner_text">{tt('themeSetting.bannerText')}</label>
        <input
          id="theme-setting-banner_text"
          type="text"
          value={draft.content.banner_text}
          onChange={(event) => {
            onChange('banner_text', event.target.value);
          }}
        />
      </p>
      {help('banner_text')}
      <HtmlEditor
        id="theme-setting-footer_text"
        label={tt('themeSetting.footerText')}
        value={draft.content.footer_text}
        onChange={(value) => {
          onChange('footer_text', value);
        }}
      />
      {help('footer_text')}
    </fieldset>
  );
};

/** Export, import and reset: the theme as a file, and the way back to the preset. */
const ThemeFileActions = ({
  draft,
  siteSlug,
  onImport,
  onReset,
}: {
  readonly draft: ThemeDraft;
  readonly siteSlug: string;
  readonly onImport: (draft: ThemeDraft | null) => void;
  readonly onReset: () => void;
}): ReactElement => {
  const tt = useThemeText();
  const file = useRef<HTMLInputElement | null>(null);
  const download = (): void => {
    const url = URL.createObjectURL(new Blob([exportTheme(draft)], { type: 'application/json' }));
    const link = document.createElement('a');
    link.href = url;
    link.download = `${siteSlug}-theme.json`;
    link.click();
    URL.revokeObjectURL(url);
  };
  return (
    <p className="theme-file-actions">
      <button type="button" onClick={download}>
        {tt('theme.export')}
      </button>
      <button
        type="button"
        onClick={() => {
          file.current?.click();
        }}
      >
        {tt('theme.import')}
      </button>
      <input
        ref={file}
        type="file"
        accept="application/json,.json"
        hidden
        aria-label={tt('theme.import')}
        onChange={(event) => {
          const chosen = event.target.files?.[0];
          event.target.value = '';
          if (chosen !== undefined) {
            void chosen.text().then((text) => {
              onImport(importTheme(text, draft));
            });
          }
        }}
      />
      <button type="button" onClick={onReset} disabled={Object.keys(draft.overrides).length === 0}>
        {tt('theme.resetPreset')}
      </button>
    </p>
  );
};

const ThemeEditor = ({ siteId, site }: { readonly siteId: SiteId; readonly site: Site }): ReactElement => {
  const { t } = useTranslator(common, sites);
  const tt = useThemeText();
  const [draft, setDraft] = useState<ThemeDraft>(() => draftFromStored(storedSettings(site)));
  const [blocks, setBlocks] = useState<ResourcePageBlocksDraft>(() =>
    resourcePageBlockDrafts(RESOURCE_PAGE_BLOCKS_REGISTRY, storedSettings(site)),
  );
  const [tab, setTab] = useState<Tab>('design');
  const [page, setPage] = useState<SitePage | null>(null);
  const [target, setTarget] = useState<PreviewTarget>('page');
  const sample = useSample(siteId, target);
  const [preview, setPreview] = useState<PreviewState>({ html: null, status: 'idle', message: null });
  const [cssErrors, setCssErrors] = useState<readonly CssRefusal[]>([]);
  const [notice, setNotice] = useState<'imported' | 'importError' | null>(null);
  const { state: saveState, save } = useSave();

  const resolved = useMemo(() => resolve(draft), [draft]);
  const checks = useMemo(() => contrastChecks(resolved), [resolved]);
  const problems = useMemo(() => draftProblems(draft), [draft]);
  const settings = useMemo(() => draftWire(draft), [draft]);
  const withBlocks = useMemo(
    () => ({
      ...settings,
      resource_page_blocks: resourcePageBlocksWire(RESOURCE_PAGE_BLOCKS_REGISTRY, blocks),
    }),
    [settings, blocks],
  );
  const blocked = problems.badHex || problems.failing > 0 || cssErrors.length > 0;
  const unsaved = useUnsavedTab(withBlocks);

  const update = useCallback((change: (current: ThemeDraft) => ThemeDraft): void => {
    setDraft(change);
    setNotice(null);
  }, []);

  // The homepage, or the first page, is what the preview renders.
  useEffect(() => {
    const id = site.homepage?.id ?? site.pages[0]?.id;
    if (id === undefined) {
      setPage(null);
      return;
    }
    let cancelled = false;
    api.getSitePage(id).then(
      (loaded) => {
        if (!cancelled) {
          setPage(loaded);
        }
      },
      () => {
        if (!cancelled) {
          setPage(null);
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, [site]);

  // A short debounce keeps a slider drag or a burst of typing from firing one
  // request per change. A refused CSS draft keeps the last good render and
  // marks the CSS instead.
  // A resource target sends the section order too; the page target leaves it
  // out, so a section edit does not re-render a page that cannot show it.
  const body = useMemo(() => {
    if (sample === null) {
      return page === null ? null : previewBody(page, settings);
    }
    return sample.status === 'ready' ? resourcePreviewBody(sample, withBlocks) : null;
  }, [page, sample, settings, withBlocks]);

  useEffect(() => {
    if (body === null) {
      setPreview((current) => ({ ...current, html: null, status: 'idle' }));
      return;
    }
    setPreview((current) => ({ ...current, status: 'loading' }));
    const timer = setTimeout(() => {
      requestPreview(site.slug, body).then(
        (result) => {
          if (result.kind === 'html') {
            setCssErrors([]);
            setPreview({ html: result.html, status: 'ready', message: null });
          } else if (result.kind === 'unavailable') {
            setPreview({ html: null, status: 'unavailable', message: null });
          } else if (cssRefusals(result.errors, result.sourceErrors).length > 0) {
            setCssErrors(cssRefusals(result.errors, result.sourceErrors));
            setPreview((current) => ({ ...current, status: 'ready' }));
          } else {
            setPreview({ html: null, status: 'error', message: result.message });
          }
        },
        (error: unknown) => {
          setPreview({
            html: null,
            status: 'error',
            message: error instanceof Error ? error.message : String(error),
          });
        },
      );
    }, 300);
    return () => {
      clearTimeout(timer);
    };
  }, [body, site.slug]);

  const submit = (event: { preventDefault: () => void }): void => {
    event.preventDefault();
    if (blocked) {
      return;
    }
    save(async () => {
      try {
        await api.patchSite(siteId, encodeSiteThemeSettings(withBlocks));
        unsaved.saved();
      } catch (error: unknown) {
        if (error instanceof ApiError && cssRefusals(error.errors, error.sourceErrors).length > 0) {
          setCssErrors(cssRefusals(error.errors, error.sourceErrors));
          setTab('css');
        }
        throw error;
      }
    });
  };

  const tabButton = (key: Tab, label: string): ReactElement => (
    <button
      type="button"
      role="tab"
      id={`theme-tab-${key}`}
      aria-selected={tab === key}
      aria-controls={`theme-panel-${key}`}
      onClick={() => {
        setTab(key);
      }}
    >
      {label}
      {key === 'css' && cssErrors.length > 0 ? <span className="theme-tab-badge">{cssErrors.length}</span> : null}
    </button>
  );

  return (
    <div className="theme-editor">
      <form onSubmit={submit}>
        <div className="theme-tabs" role="tablist" aria-label={tt('theme.sections')}>
          {tabButton('design', tt('theme.design'))}
          {tabButton('css', tt('theme.customCssTab'))}
        </div>
        {tab === 'design' ? (
          <div role="tabpanel" id="theme-panel-design" aria-labelledby="theme-tab-design">
            <PresetGallery
              draft={draft}
              onApply={(preset) => {
                update((current) => applyPreset(current, preset));
              }}
            />
            <BrandPanel
              draft={draft}
              resolved={resolved}
              onBrand={(value) => {
                update((current) => setOverride(current, 'brand_color', value));
              }}
              onDerive={(derive) => {
                update((current) => ({ ...current, derive }));
              }}
            />
            <ContrastPanel
              checks={checks}
              onUse={(role, colour, shade) => {
                const entry = THEME_REGISTRY.colours.find((colourEntry) => colourEntry.role === role);
                // Under the automatic scheme a dark pair is fixed by the dark value.
                const key =
                  resolved.scheme === 'auto' && shade === 'dark' ? entry?.darkKey : entry?.key;
                if (key !== undefined && key !== null) {
                  update((current) => setOverride(current, key, colour));
                }
              }}
            />
            <FineTune
              draft={draft}
              resolved={resolved}
              onSet={(key, value) => {
                update((current) => setOverride(current, key, value));
              }}
            />
            <ContentSection
              draft={draft}
              onChange={(key, value) => {
                update((current) => ({ ...current, content: { ...current.content, [key]: value } }));
              }}
            />
            <ResourcePageBlocksField
              registry={RESOURCE_PAGE_BLOCKS_REGISTRY}
              drafts={blocks}
              onChange={setBlocks}
            />
            <ThemeFileActions
              draft={draft}
              siteSlug={site.slug}
              onImport={(imported) => {
                if (imported === null) {
                  setNotice('importError');
                } else {
                  setDraft(imported);
                  setNotice('imported');
                }
              }}
              onReset={() => {
                update((current) => applyPreset(current, current.preset));
              }}
            />
            {notice === null ? null : (
              <p className={notice === 'imported' ? 'field-note' : 'field-error'} role="status">
                {tt(notice === 'imported' ? 'theme.imported' : 'theme.importError')}
              </p>
            )}
          </div>
        ) : (
          <div role="tabpanel" id="theme-panel-css" aria-labelledby="theme-tab-css">
            <Suspense fallback={<p role="status">{t('loading.page')}</p>}>
              <CustomCssTab
                value={draft.customCss}
                maxBytes={CSS_MAX_BYTES}
                errors={cssErrors}
                previewHtml={preview.html}
                onChange={(customCss) => {
                  update((current) => ({ ...current, customCss }));
                }}
              />
            </Suspense>
          </div>
        )}
        {blocked ? (
          <p className="field-error" role="alert">
            {tt('theme.blocked')}
          </p>
        ) : null}
        <SaveStatus state={saveState} />
        <SaveBar dirty={unsaved.dirty}>
          <p className="form-actions">
            <button type="submit" disabled={blocked || saveState.status === 'saving'}>
              {saveState.status === 'saving' ? t('field.saving') : t('action.save')}
            </button>
          </p>
        </SaveBar>
      </form>
      <PreviewPanel
        html={preview.html}
        status={preview.status}
        message={preview.message}
        controls={<PreviewSwitch value={target} onChange={setTarget} />}
        {...(sample?.status === 'none' ? { empty: t(NONE_KEYS[sample.kind]) } : {})}
      />
    </div>
  );
};

/** Loads the site and its theme settings, then renders the editor. */
export const SiteThemePage = ({ siteId }: { readonly siteId: SiteId }): ReactElement => {
  const { t } = useTranslator(common, sites);
  const state = useAsync(() => api.getSite(siteId), [siteId]);
  const access = useSiteAccess();
  // The page's own table suspends the route until it arrives, so no label renders in English.
  useThemeText();
  if (access?.has('changeSite') === false) {
    return <AccessNote>{t('siteAccess.theme')}</AccessNote>;
  }
  if (state.status === 'loading' || access === null) {
    return <p role="status">{t('loading.site')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return <ThemeEditor key={String(siteId)} siteId={siteId} site={state.data} />;
};
