/**
 * The site Theme page: the design tokens a curator edits, a live WCAG contrast
 * readout, and a live preview of the site's homepage rendered with the unsaved
 * tokens.
 *
 * The controls come from the generated schema in
 * `src/generated/themeSettings.json`, written by
 * `crates/omeka-site/src/theme/tokens.rs`. A save PATCHes the site's
 * `o:theme_settings` map; a preview POSTs the same map to
 * `/s/<slug>/preview`, which renders it and stores nothing.
 *
 * The contrast gate is the reason this page exists beside the generic settings
 * form: a palette below AA 4.5:1 cannot be saved.
 */
import type { ReactElement } from 'react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { api } from '../../api/client';
import { isRecord } from '../../api/parse';
import type { JsonValue, Site, SiteId, SitePage } from '../../api/types';
import { encodeSiteThemeSettings } from '../../api/write';
import { ErrorNotice } from '../../app/errors';
import { SaveStatus } from '../../app/fields';
import { useSave } from '../../app/save';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import type { MessageKey } from '../../i18n';
import { AssetPicker } from './editor/fields/AssetPicker';
import { HtmlEditor } from './editor/fields/HtmlEditor';
import { PreviewPanel } from './editor/shell/PreviewPanel';
import type { PreviewStatus } from './editor/shell/PreviewPanel';
import { requestPreview } from './editor/preview';
import { ResourcePageBlocksField } from './ResourcePageBlocksField';
import { RESOURCE_PAGE_BLOCKS_REGISTRY } from './resourcePageBlocksRegistry';
import { resourcePageBlockDrafts, resourcePageBlocksWire } from './resourcePageBlocksSchema';
import type { ResourcePageBlocksDraft } from './resourcePageBlocksSchema';
import { THEME_SETTINGS_REGISTRY } from './themeSettingsRegistry';
import {
  isHexColour,
  themeSettingDrafts,
  themeSettingsWire,
} from './themeSettingsSchema';
import type { ThemeSettingDraft, ThemeSettingSpec } from './themeSettingsSchema';
import { contrastChecks, formatRatio } from './contrast';

/** The control groups, in the order the page shows them. */
const GROUPS: readonly { readonly titleKey: MessageKey; readonly keys: readonly string[] }[] = [
  {
    titleKey: 'theme.palette',
    keys: [
      'color_scheme',
      'brand_color',
      'accent_color',
      'background_color',
      'surface_color',
      'text_color',
      'muted_color',
      'link_color',
    ],
  },
  {
    titleKey: 'theme.typography',
    keys: ['heading_font', 'body_font', 'base_font_size', 'type_ratio'],
  },
  { titleKey: 'theme.layout', keys: ['density', 'corner_radius', 'header_layout'] },
  { titleKey: 'theme.headerFooter', keys: ['logo', 'banner', 'banner_text', 'footer_text'] },
];

/** A friendlier label than the stored value, for the known choices. */
const CHOICE_LABELS: Readonly<Record<string, string>> = {
  light: 'Light',
  dark: 'Dark',
  auto: 'Automatic',
  compact: 'Compact',
  comfortable: 'Comfortable',
  spacious: 'Spacious',
  'logo-left': 'Logo left',
  centred: 'Centred',
  'system-sans': 'System sans',
  'system-serif': 'System serif',
  humanist: 'Humanist sans',
  geometric: 'Geometric sans',
  'transitional-serif': 'Transitional serif',
  mono: 'Monospace',
};

const choiceLabel = (value: string): string => CHOICE_LABELS[value] ?? value;

const storedSettings = (site: Site): Readonly<Record<string, unknown>> => {
  const value = site.themeSettings;
  return isRecord(value) ? value : {};
};

const ColourControl = ({
  spec,
  value,
  onChange,
}: {
  readonly spec: Extract<ThemeSettingSpec, { readonly kind: 'colour' }>;
  readonly value: string;
  readonly onChange: (value: string) => void;
}): ReactElement => {
  const { t } = useTranslator();
  const id = `theme-setting-${spec.key}`;
  const invalid = value !== '' && !isHexColour(value);
  return (
    <p className="theme-colour">
      <label htmlFor={id}>{t(spec.labelKey)}</label>
      <input
        id={`${id}-swatch`}
        type="color"
        aria-label={t(spec.labelKey)}
        value={isHexColour(value) ? value : spec.default}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      />
      <input
        id={id}
        type="text"
        spellCheck={false}
        value={value}
        aria-invalid={invalid}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      />
      {invalid ? (
        <span className="field-note" role="alert">
          {t('theme.hexHint')}
        </span>
      ) : null}
    </p>
  );
};

/** One control per token kind. */
const TokenControl = ({
  spec,
  value,
  onChange,
}: {
  readonly spec: ThemeSettingSpec;
  readonly value: ThemeSettingDraft;
  readonly onChange: (value: ThemeSettingDraft) => void;
}): ReactElement => {
  const { t } = useTranslator();
  const id = `theme-setting-${spec.key}`;
  if (spec.kind === 'colour') {
    return (
      <ColourControl
        spec={spec}
        value={typeof value === 'string' ? value : spec.default}
        onChange={onChange}
      />
    );
  }
  if (spec.kind === 'font' || spec.kind === 'choice') {
    return (
      <p>
        <label htmlFor={id}>{t(spec.labelKey)}</label>
        <select
          id={id}
          value={typeof value === 'string' ? value : spec.default}
          onChange={(event) => {
            onChange(event.target.value);
          }}
        >
          {spec.choices.map((choice) => (
            <option key={choice} value={choice}>
              {choiceLabel(choice)}
            </option>
          ))}
        </select>
      </p>
    );
  }
  if (spec.kind === 'number') {
    const current = typeof value === 'number' ? value : spec.default;
    return (
      <p>
        <label htmlFor={id}>{t(spec.labelKey)}</label>
        <input
          id={`${id}-range`}
          type="range"
          min={spec.min}
          max={spec.max}
          step={spec.step}
          aria-label={t(spec.labelKey)}
          value={current}
          onChange={(event) => {
            onChange(Number(event.target.value));
          }}
        />
        <input
          id={id}
          type="number"
          min={spec.min}
          max={spec.max}
          step={spec.step}
          value={current}
          onChange={(event) => {
            onChange(Number(event.target.value));
          }}
        />
        {spec.key === 'type_ratio' ? (
          <span className="field-note">{(current / 100).toFixed(2)}</span>
        ) : null}
      </p>
    );
  }
  if (spec.kind === 'asset') {
    return (
      <AssetPicker
        id={id}
        label={t(spec.labelKey)}
        value={typeof value === 'string' ? value : ''}
        onChange={onChange}
      />
    );
  }
  if (spec.kind === 'html') {
    return (
      <HtmlEditor
        id={id}
        label={t(spec.labelKey)}
        value={typeof value === 'string' ? value : ''}
        onChange={onChange}
      />
    );
  }
  return (
    <p>
      <label htmlFor={id}>{t(spec.labelKey)}</label>
      <input
        id={id}
        type="text"
        value={typeof value === 'string' ? value : ''}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      />
    </p>
  );
};

/** The AA readout for the three pairs the palette is checked against. */
const ContrastReadout = ({
  checks,
}: {
  readonly checks: ReturnType<typeof contrastChecks>;
}): ReactElement => {
  const { t } = useTranslator();
  return (
    <section className="theme-contrast" aria-label={t('theme.contrast')}>
      <h3>{t('theme.contrast')}</h3>
      <ul>
        {checks.map((check) => (
          <li
            key={check.key}
            className={check.passes ? 'contrast-pass' : 'contrast-fail'}
            data-contrast={check.key}
            data-pass={String(check.passes)}
          >
            <span className="contrast-label">{t(check.labelKey)}</span>
            <span className="contrast-ratio">{formatRatio(check.ratio)}</span>
            <span className="contrast-state">
              {check.passes ? t('theme.contrastPass') : t('theme.contrastFail')}
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
};

/** The preview request body: the stored page plus the unsaved tokens. */
const previewBody = (
  page: SitePage,
  tokens: Readonly<Record<string, JsonValue>>,
): Readonly<Record<string, unknown>> => ({
  'o:id': page.id,
  'o:title': page.title,
  'o:slug': page.slug,
  'o:is_public': page.isPublic,
  'o:layout': page.layout ?? '',
  'o:layout_data': page.layoutData,
  'o:theme_settings': tokens,
});

interface PreviewState {
  readonly html: string | null;
  readonly status: PreviewStatus;
  readonly message: string | null;
}

/** A token's wire value as a string, for the contrast checks. */
const wireColour = (tokens: Readonly<Record<string, JsonValue>>, key: string): string => {
  const value = tokens[key];
  return typeof value === 'string' ? value : '';
};

const ThemeEditor = ({
  siteId,
  site,
}: {
  readonly siteId: SiteId;
  readonly site: Site;
}): ReactElement => {
  const { t } = useTranslator();
  const [drafts, setDrafts] = useState<Readonly<Record<string, ThemeSettingDraft>>>(() =>
    themeSettingDrafts(THEME_SETTINGS_REGISTRY, storedSettings(site)),
  );
  const [blocks, setBlocks] = useState<ResourcePageBlocksDraft>(() =>
    resourcePageBlockDrafts(RESOURCE_PAGE_BLOCKS_REGISTRY, storedSettings(site)),
  );
  const [page, setPage] = useState<SitePage | null>(null);
  const [preview, setPreview] = useState<PreviewState>({
    html: null,
    status: 'idle',
    message: null,
  });
  const { state: saveState, save } = useSave();

  const set = useCallback((key: string, value: ThemeSettingDraft): void => {
    setDrafts((current) => ({ ...current, [key]: value }));
  }, []);

  const tokens = useMemo(
    () => themeSettingsWire(THEME_SETTINGS_REGISTRY, drafts),
    [drafts],
  );
  const checks = useMemo(
    () =>
      contrastChecks({
        text: wireColour(tokens, 'text_color'),
        background: wireColour(tokens, 'background_color'),
        link: wireColour(tokens, 'link_color'),
        brand: wireColour(tokens, 'brand_color'),
      }),
    [tokens],
  );
  const invalidHex = THEME_SETTINGS_REGISTRY.settings.some(
    (spec) => spec.kind === 'colour' && typeof drafts[spec.key] === 'string' && !isHexColour(drafts[spec.key]),
  );
  const blocked = invalidHex || checks.some((check) => !check.passes);

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

  // A short debounce keeps a slider drag from firing one request per pixel.
  useEffect(() => {
    if (page === null) {
      return;
    }
    setPreview((current) => ({ ...current, status: 'loading' }));
    const timer = setTimeout(() => {
      requestPreview(site.slug, previewBody(page, tokens)).then(
        (result) => {
          if (result.kind === 'html') {
            setPreview({ html: result.html, status: 'ready', message: null });
          } else if (result.kind === 'unavailable') {
            setPreview({ html: null, status: 'unavailable', message: null });
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
  }, [page, tokens, site.slug]);

  const submit = (event: { preventDefault: () => void }): void => {
    event.preventDefault();
    if (blocked) {
      return;
    }
    save(async () => {
      const settings: Readonly<Record<string, JsonValue>> = {
        ...tokens,
        resource_page_blocks: resourcePageBlocksWire(RESOURCE_PAGE_BLOCKS_REGISTRY, blocks),
      };
      await api.patchSite(siteId, encodeSiteThemeSettings(settings));
    });
  };

  const specFor = (key: string): ThemeSettingSpec | undefined =>
    THEME_SETTINGS_REGISTRY.settings.find((spec) => spec.key === key);

  return (
    <div className="theme-editor">
      <form onSubmit={submit}>
        {GROUPS.map((group) => (
          <fieldset key={group.titleKey}>
            <legend>{t(group.titleKey)}</legend>
            {group.keys.map((key) => {
              const spec = specFor(key);
              if (spec === undefined) {
                return null;
              }
              return (
                <TokenControl
                  key={key}
                  spec={spec}
                  value={drafts[key] ?? ''}
                  onChange={(value) => {
                    set(key, value);
                  }}
                />
              );
            })}
          </fieldset>
        ))}
        <ContrastReadout checks={checks} />
        <ResourcePageBlocksField
          registry={RESOURCE_PAGE_BLOCKS_REGISTRY}
          drafts={blocks}
          onChange={setBlocks}
        />
        {blocked ? (
          <p className="field-error" role="alert">
            {t('theme.contrastBlocked')}
          </p>
        ) : null}
        <SaveStatus state={saveState} />
        <p className="form-actions">
          <button type="submit" disabled={blocked || saveState.status === 'saving'}>
            {saveState.status === 'saving' ? t('field.saving') : t('action.save')}
          </button>
        </p>
      </form>
      <PreviewPanel html={preview.html} status={preview.status} message={preview.message} />
    </div>
  );
};

/** Loads the site and its tokens, then renders the editor. */
export const SiteThemePage = ({ siteId }: { readonly siteId: SiteId }): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(() => api.getSite(siteId), [siteId]);
  if (state.status === 'loading') {
    return <p role="status">{t('loading.site')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return <ThemeEditor key={String(siteId)} siteId={siteId} site={state.data} />;
};
