/**
 * The preset gallery: one card per preset, each with a small drawing of the
 * page in that preset's colours, type and header, so the curator sees the
 * look before choosing it. One click applies it.
 *
 * The drawing is plain markup styled from the resolved tokens, not a server
 * render: five previews would cost five authenticated requests, and the
 * drawing needs only the palette and the few tokens it shows.
 */
import type { CSSProperties, ReactElement } from 'react';
import { useThemeText } from './messages';
import { onBrand } from './palette';
import type { Palette } from './palette';
import { THEME_REGISTRY } from './registry';
import type { Preset } from './registry';
import { resolve } from './draft';
import type { ThemeDraft } from './draft';

/** The CSS `font-family` of a font key, the stacks the theme ships. */
const FONT_STACKS: Readonly<Record<string, string>> = {
  'system-sans': 'system-ui, sans-serif',
  'system-serif': 'Georgia, serif',
  humanist: '"Gill Sans", "Trebuchet MS", Calibri, sans-serif',
  geometric: 'Futura, "Century Gothic", "Avenir Next", sans-serif',
  'transitional-serif': 'Charter, Cambria, serif',
  didone: 'Didot, "Bodoni 72", "Bodoni MT", Georgia, serif',
  mono: 'ui-monospace, Menlo, monospace',
  fraunces: 'Fraunces, Georgia, serif',
  'source-sans-3': '"Source Sans 3", system-ui, sans-serif',
  'ibm-plex-sans': '"IBM Plex Sans", system-ui, sans-serif',
  inter: 'Inter, system-ui, sans-serif',
  'ibm-plex-mono': '"IBM Plex Mono", ui-monospace, monospace',
};

const fontStack = (key: string | number | undefined): string =>
  FONT_STACKS[String(key)] ?? 'system-ui, sans-serif';

const headerStyle = (style: string, palette: Palette): CSSProperties => {
  switch (style) {
    case 'band':
      return { background: palette.brand, color: onBrand(palette) };
    case 'rule':
      return { borderBottom: `3px double ${palette.text}` };
    case 'minimal':
      return {};
    default:
      return { borderBottom: `1px solid color-mix(in srgb, ${palette.text} 18%, ${palette.background})` };
  }
};

const buttonStyle = (style: string, palette: Palette): CSSProperties => {
  switch (style) {
    case 'solid':
      return { background: palette.brand, borderColor: palette.brand };
    case 'outline':
      return { background: 'transparent', borderColor: palette.link };
    default:
      return { background: palette.surface, borderColor: palette.muted };
  }
};

/** The drawing of one preset's page. Decorative: the card's name labels it. */
const Miniature = ({ preset, derive }: { readonly preset: Preset; readonly derive: boolean }): ReactElement => {
  const resolved = resolve({
    preset: preset.id,
    derive,
    overrides: {},
    content: { logo: '', banner: '', banner_text: '' },
    footer: { columns: [], links: null, contact: '', licence: 'none', poweredBy: true },
    customCss: '',
  });
  const palette = resolved.scheme === 'dark' ? resolved.dark : resolved.light;
  const radius = `${String(Math.min(Number(resolved.tokens['corner_radius']), 8))}px`;
  return (
    <div
      className="theme-mini"
      aria-hidden="true"
      style={{ background: palette.background, color: palette.text, fontFamily: fontStack(resolved.tokens['body_font']) }}
    >
      <div
        className="theme-mini-header"
        data-centred={resolved.tokens['header_layout'] === 'centred'}
        style={headerStyle(String(resolved.tokens['header_style']), palette)}
      >
        <span style={{ fontFamily: fontStack(resolved.tokens['heading_font']) }}>Aa</span>
        <i style={{ background: 'currentColor' }} />
      </div>
      <div className="theme-mini-body">
        <b style={{ background: palette.text }} />
        <b style={{ background: palette.muted, width: '70%' }} />
        <b style={{ background: palette.link, width: '40%' }} />
        <div className="theme-mini-card" style={{ background: palette.surface, borderRadius: radius }}>
          <span
            className="theme-mini-button"
            style={{ ...buttonStyle(String(resolved.tokens['button_style']), palette), borderRadius: radius }}
          />
        </div>
      </div>
    </div>
  );
};

export const PresetGallery = ({
  draft,
  onApply,
}: {
  readonly draft: ThemeDraft;
  readonly onApply: (preset: string) => void;
}): ReactElement => {
  const t = useThemeText();
  return (
    <section className="theme-section" aria-labelledby="theme-presets">
      <h3 id="theme-presets">{t('theme.presets')}</h3>
      <p className="field-note">{t('theme.presetsHelp')}</p>
      <ul className="theme-presets">
        {THEME_REGISTRY.presets.map((preset) => (
          <li key={preset.id}>
            <button
              type="button"
              className="theme-preset"
              aria-pressed={draft.preset === preset.id}
              onClick={() => {
                onApply(preset.id);
              }}
            >
              <Miniature preset={preset} derive={draft.derive} />
              <span className="theme-preset-name">{t(preset.labelKey)}</span>
              <span className="theme-preset-description">{t(preset.descriptionKey)}</span>
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
};
