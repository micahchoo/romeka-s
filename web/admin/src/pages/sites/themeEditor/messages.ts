/**
 * The Theme page's messages.
 *
 * They live beside the page rather than in `src/i18n/catalogue.ts` because the
 * English catalogue is part of the first paint, and the page's 150-odd labels,
 * help lines and choice names would take that payload over its budget
 * (`perf/budget.json`, `PERF.md`). English is here, in the page chunk; every
 * other locale is its own chunk under `locales/`, loaded through
 * [`THEME_LOADERS`] like the main catalogue's `CATALOGUE_LOADERS`. The
 * guarantees are the main catalogue's: each table is typed against `en`, so a
 * key added in English and missing elsewhere is a `tsc` error, a locale with
 * no loader is a `tsc` error, and `src/generated/themeSettings.json` names only
 * keys this table holds.
 */
import { createContext, useContext, useEffect, useMemo, useState } from 'react';
import { useLocale } from '../../../i18n';
import type { Locale, MessageParams } from '../../../i18n';

export const en = {
  'theme.design': 'Design',
  'theme.customCssTab': 'Custom CSS',
  'theme.sections': 'Theme sections',
  'theme.presets': 'Start from a preset',
  'theme.presetsHelp': 'One click applies every value. Your header, footer and custom CSS stay.',
  'themePreset.plain': 'Plain',
  'themePreset.plainDescription': 'The original look: system type and a soft blue.',
  'themePreset.archive': 'Archive',
  'themePreset.archiveDescription': 'Warm paper, serif type and a centred masthead.',
  'themePreset.gallery': 'Gallery',
  'themePreset.galleryDescription': 'Dark and image-first, with almost no chrome.',
  'themePreset.editorial': 'Editorial',
  'themePreset.editorialDescription': 'Strong serif headings over a narrow column.',
  'themePreset.civic': 'Civic',
  'themePreset.civicDescription': 'A bright brand band and clear sans type.',
  'theme.palette': 'Palette',
  'theme.onBrand': 'Text on brand',
  'theme.contrast': 'Contrast',
  'theme.contrastPass': 'Every text and background pair passes WCAG AA (4.5:1).',
  'theme.contrastFail': 'Pairs below AA 4.5:1: {count}. Save waits until they pass.',
  'theme.contrastPair': '{foreground} on {background}',
  'theme.contrastUse': 'Use {colour}',
  'theme.contrastUseLabel': 'Use {colour} for {role}',
  'theme.hexHint': 'Use a hex colour, e.g. #1a5d8f.',
  'theme.fineTune': 'Fine-tune',
  'theme.fineTuneHelp': 'Change single values. A changed value shows a Reset button.',
  'theme.colours': 'Colours',
  'theme.typography': 'Typography',
  'theme.layout': 'Layout',
  'theme.headerFooter': 'Header and footer',
  'theme.reset': 'Reset',
  'theme.resetLabel': 'Reset {label} to the preset value',
  'theme.resetPreset': 'Reset to preset',
  'theme.export': 'Export theme',
  'theme.import': 'Import theme',
  'theme.importError': 'That file is not a theme exported from this page.',
  'theme.imported': 'Theme imported. Save to keep it.',
  'theme.blocked': 'Fix the problems above before saving.',
  'theme.cssErrors': 'Problems in your CSS. It is not saved until they are fixed.',
  'theme.cssSize': '{used} of {max} bytes',
  'theme.themeCss': 'Theme CSS',
  'theme.themeCssHelp': 'Read only. Your CSS loads after these rules, so your rules win.',
  'theme.variables': 'Variables',
  'theme.variable': 'Variable',
  'theme.value': 'Value',
  'theme.meaning': 'Meaning',
  'theme.stylesheet': 'Full theme stylesheet',
  'theme.copy': 'Copy',
  'theme.copied': 'Copied.',
  'theme.variablesWait': 'The values appear when the preview loads.',
  'theme.varBorder': 'Rules and borders: the text colour mixed into the background.',
  'theme.varSpace': 'The spacing steps, from the density.',
  'theme.varRadiusSm': 'The smaller corner radius.',
  'theme.varFontSize': 'A text size, from the base size and the type scale.',
  'theme.varLineHeight': 'The body line height, from the density.',
  'themeSetting.preset': 'Preset',
  'themeSetting.presetHelp': 'The look the site starts from.',
  'themeSetting.derivePalette': 'Build the palette from the brand colour',
  'themeSetting.derivePaletteHelp': 'On: every colour follows from the brand and passes contrast. Off: the preset colours.',
  'themeSetting.brandColor': 'Brand colour',
  'themeSetting.brandColorHelp': 'Header bands, solid buttons and the current page mark.',
  'themeSetting.colorScheme': 'Colour scheme',
  'themeSetting.colorSchemeHelp': 'Light, dark, or follow the reader\'s system.',
  'themeSetting.accentColor': 'Accent colour',
  'themeSetting.accentColorHelp': 'Small emphasis, such as the advanced search link.',
  'themeSetting.backgroundColor': 'Page background',
  'themeSetting.backgroundColorHelp': 'Behind all page content.',
  'themeSetting.surfaceColor': 'Surface',
  'themeSetting.surfaceColorHelp': 'Cards, the banner and other inset areas.',
  'themeSetting.textColor': 'Text',
  'themeSetting.textColorHelp': 'Body text and headings.',
  'themeSetting.mutedColor': 'Muted text',
  'themeSetting.mutedColorHelp': 'Captions, dates and other secondary text.',
  'themeSetting.linkColor': 'Link',
  'themeSetting.linkColorHelp': 'Links in the page text.',
  'themeSetting.headingFont': 'Heading font',
  'themeSetting.headingFontHelp': 'Headings and the site title.',
  'themeSetting.bodyFont': 'Body font',
  'themeSetting.bodyFontHelp': 'All other text.',
  'themeSetting.baseFontSize': 'Base font size (px)',
  'themeSetting.baseFontSizeHelp': 'The body text size. Headings scale from it.',
  'themeSetting.typeRatio': 'Type scale ratio',
  'themeSetting.typeRatioHelp': 'How much larger each heading level is than the next.',
  'themeSetting.density': 'Density',
  'themeSetting.densityHelp': 'The space around and between content.',
  'themeSetting.cornerRadius': 'Corner radius (px)',
  'themeSetting.cornerRadiusHelp': 'How round corners are.',
  'themeSetting.contentWidth': 'Content width',
  'themeSetting.contentWidthHelp': 'How wide the content and header grow on a large screen.',
  'themeSetting.headerLayout': 'Header layout',
  'themeSetting.headerLayoutHelp': 'Where the logo, title and menu sit.',
  'themeSetting.headerStyle': 'Header style',
  'themeSetting.headerStyleHelp': 'How the header stands apart from the page.',
  'themeSetting.buttonStyle': 'Button style',
  'themeSetting.buttonStyleHelp': 'How buttons look.',
  'themeSetting.logo': 'Logo',
  'themeSetting.logoHelp': 'An image before the site title.',
  'themeSetting.banner': 'Banner image',
  'themeSetting.bannerHelp': 'A wide image above the header.',
  'themeSetting.bannerText': 'Banner text',
  'themeSetting.bannerTextHelp': 'A short line in the banner.',
  'themeSetting.footerText': 'Footer text',
  'themeSetting.footerTextHelp': 'Text at the foot of every page.',
  'themeSetting.customCss': 'Custom CSS',
  'themeSetting.customCssHelp': 'Loads after the theme, so your rules win. url() may only name this site\'s /files/ and /assets/.',
  'themeChoice.light': 'Light',
  'themeChoice.dark': 'Dark',
  'themeChoice.auto': 'Automatic',
  'themeChoice.compact': 'Compact',
  'themeChoice.comfortable': 'Comfortable',
  'themeChoice.spacious': 'Spacious',
  'themeChoice.narrow': 'Narrow',
  'themeChoice.standard': 'Standard',
  'themeChoice.wide': 'Wide',
  'themeChoice.logoLeft': 'Logo left',
  'themeChoice.centred': 'Centred',
  'themeChoice.plain': 'Thin rule',
  'themeChoice.band': 'Brand band',
  'themeChoice.rule': 'Double rule',
  'themeChoice.minimal': 'Minimal',
  'themeChoice.soft': 'Soft',
  'themeChoice.solid': 'Solid',
  'themeChoice.outline': 'Outline',
  'themeChoice.systemSans': 'System sans',
  'themeChoice.systemSerif': 'System serif',
  'themeChoice.humanist': 'Humanist sans',
  'themeChoice.geometric': 'Geometric sans',
  'themeChoice.transitionalSerif': 'Transitional serif',
  'themeChoice.didone': 'High-contrast serif',
  'themeChoice.mono': 'Monospace',
} as const;

/** A Theme page message id. */
export type ThemeMessageKey = keyof typeof en;

/** A complete Theme page table: every message id, no fallback at read time. */
export type ThemeMessages = Readonly<Record<ThemeMessageKey, string>>;

/** How each locale's table arrives. `Record<Locale, …>` makes a missing locale a `tsc` error. */
const THEME_LOADERS: Readonly<Record<Locale, () => Promise<ThemeMessages>>> = {
  en: () => Promise.resolve(en),
  es: () => import('./locales/es').then((module) => module.es),
  hi: () => import('./locales/hi').then((module) => module.hi),
  mr: () => import('./locales/mr').then((module) => module.mr),
  ml: () => import('./locales/ml').then((module) => module.ml),
  ta: () => import('./locales/ta').then((module) => module.ta),
  kn: () => import('./locales/kn').then((module) => module.kn),
  te: () => import('./locales/te').then((module) => module.te),
  ne: () => import('./locales/ne').then((module) => module.ne),
};

const loaded = new Map<Locale, ThemeMessages>([['en', en]]);

/**
 * The active locale's table, or `null` until the first one arrives. A later
 * locale switch keeps the previous table until the new one loads, as the main
 * store keeps its translator, so the page never unmounts a draft to wait.
 */
export const useThemeMessages = (): ThemeMessages | null => {
  const locale = useLocale();
  const [table, setTable] = useState<ThemeMessages | null>(() => loaded.get(locale) ?? null);
  useEffect(() => {
    const cached = loaded.get(locale);
    if (cached !== undefined) {
      setTable(cached);
      return undefined;
    }
    let current = true;
    void THEME_LOADERS[locale]().then((messages) => {
      loaded.set(locale, messages);
      if (current) {
        setTable(messages);
      }
    });
    return () => {
      current = false;
    };
  }, [locale]);
  return table;
};

/** The table the page renders with. English outside a provider, as in a component test. */
export const ThemeMessagesContext = createContext<ThemeMessages>(en);

/** Every Theme page message id. */
export const THEME_MESSAGE_KEYS: ReadonlySet<string> = new Set(Object.keys(en));

/** Whether a string is a Theme page message id. */
export const isThemeMessageKey = (value: string): value is ThemeMessageKey =>
  THEME_MESSAGE_KEYS.has(value);

const interpolate = (template: string, params: MessageParams | undefined): string =>
  params === undefined
    ? template
    : template.replace(/\{([A-Za-z][A-Za-z0-9]*)\}/g, (match, name: string) => {
        const value = params[name];
        return value === undefined ? match : String(value);
      });

/** The Theme page's message lookup in the table [`ThemeMessagesContext`] provides. */
export const useThemeText = (): ((key: ThemeMessageKey, params?: MessageParams) => string) => {
  const messages = useContext(ThemeMessagesContext);
  return useMemo(
    () =>
      (key: ThemeMessageKey, params?: MessageParams): string =>
        interpolate(messages[key], params),
    [messages],
  );
};
