/**
 * The Fine-tune disclosure: every single token, grouped as colours, type,
 * layout and page structure, each with its help line and a Reset to the
 * preset's value.
 */
import type { ReactElement } from 'react';
import { DARK_COLOURS } from './draft';
import { useThemeText } from './messages';
import type { ThemeMessageKey } from './messages';
import { settingFor } from './registry';
import type { ThemeSetting } from './registry';
import type { Resolved, ThemeDraft, TokenValue } from './draft';
import type { ColourRole } from './palette';
import { THEME_REGISTRY } from './registry';
import { ColourInput, TokenControl, TokenRow } from './controls';

const GROUPS: readonly { readonly titleKey: ThemeMessageKey; readonly keys: readonly string[] }[] = [
  {
    titleKey: 'theme.colours',
    keys: [
      'color_scheme',
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
  {
    titleKey: 'theme.layout',
    keys: [
      'density',
      'corner_radius',
      'content_width',
      'header_layout',
      'header_style',
      'button_style',
    ],
  },
  {
    titleKey: 'theme.structure',
    keys: ['hero_style', 'card_style', 'search_display', 'item_layout', 'footer_layout', 'footer_tone'],
  },
];

/** The value a token shows: the curator's own, or what the preset resolves it to. */
const shownValue = (setting: ThemeSetting, draft: ThemeDraft, resolved: Resolved): TokenValue => {
  const own = draft.overrides[setting.key];
  if (own !== undefined) {
    return own;
  }
  if (setting.kind === 'colour') {
    const role = THEME_REGISTRY.colours.find((colour) => colour.key === setting.key)?.role;
    const palette = resolved.scheme === 'dark' ? resolved.dark : resolved.light;
    return role === undefined ? '' : palette[role];
  }
  return resolved.tokens[setting.key] ?? '';
};

const roleOf = (key: string): ColourRole | undefined =>
  THEME_REGISTRY.colours.find((colour) => colour.key === key)?.role;

const darkKeyOf = (key: string): string | undefined => {
  const role = roleOf(key);
  return DARK_COLOURS.find((colour) => colour.role === role)?.key;
};

/**
 * A colour under the automatic scheme: its light value and its dark value,
 * each with its own input. The row's label names the light input.
 */
const ShadeInputs = ({
  setting,
  darkKey,
  draft,
  resolved,
  onSet,
}: {
  readonly setting: ThemeSetting;
  readonly darkKey: string;
  readonly draft: ThemeDraft;
  readonly resolved: Resolved;
  readonly onSet: (key: string, value: TokenValue | null) => void;
}): ReactElement => {
  const t = useThemeText();
  const role = roleOf(setting.key);
  const label = t(setting.labelKey);
  const shades = [
    { shade: 'light', key: setting.key, fallback: role === undefined ? '' : resolved.light[role] },
    { shade: 'dark', key: darkKey, fallback: role === undefined ? '' : resolved.dark[role] },
  ] as const;
  return (
    <span className="theme-shades">
      {shades.map(({ shade, key, fallback }) => {
        const name = t('theme.shadeValue', { label, shade: t(shade === 'light' ? 'themeChoice.light' : 'themeChoice.dark') });
        return (
          <span key={shade} className="theme-shade">
            <span className="theme-shade-name" aria-hidden="true">
              {t(shade === 'light' ? 'themeChoice.light' : 'themeChoice.dark')}
            </span>
            <ColourInput
              id={`theme-setting-${key}`}
              label={name}
              textLabel={name}
              value={String(draft.overrides[key] ?? fallback)}
              fallback="#000000"
              onChange={(value) => {
                onSet(key, value);
              }}
            />
          </span>
        );
      })}
    </span>
  );
};

/**
 * One token's row: its control (both shades of a colour under the automatic
 * scheme), its help line and its Reset. Fine-tune lists every token this way;
 * the page editor's site region inspector lists the region's own.
 */
export const TokenField = ({
  settingKey,
  draft,
  resolved,
  onSet,
}: {
  readonly settingKey: string;
  readonly draft: ThemeDraft;
  readonly resolved: Resolved;
  readonly onSet: (key: string, value: TokenValue | null) => void;
}): ReactElement | null => {
  const setting = settingFor(settingKey);
  if (setting === undefined) {
    return null;
  }
  const darkKey = resolved.scheme === 'auto' ? darkKeyOf(settingKey) : undefined;
  return (
    <TokenRow
      setting={setting}
      changed={
        draft.overrides[settingKey] !== undefined ||
        (darkKey !== undefined && draft.overrides[darkKey] !== undefined)
      }
      onReset={() => {
        onSet(settingKey, null);
        if (darkKey !== undefined) {
          onSet(darkKey, null);
        }
      }}
    >
      {darkKey === undefined ? (
        <TokenControl
          setting={setting}
          value={shownValue(setting, draft, resolved)}
          onChange={(value) => {
            onSet(settingKey, value);
          }}
        />
      ) : (
        <ShadeInputs setting={setting} darkKey={darkKey} draft={draft} resolved={resolved} onSet={onSet} />
      )}
    </TokenRow>
  );
};

export const FineTune = ({
  draft,
  resolved,
  onSet,
}: {
  readonly draft: ThemeDraft;
  readonly resolved: Resolved;
  readonly onSet: (key: string, value: TokenValue | null) => void;
}): ReactElement => {
  const t = useThemeText();
  return (
    <details className="theme-section theme-fine-tune">
      <summary>{t('theme.fineTune')}</summary>
      <p className="field-note">{t('theme.fineTuneHelp')}</p>
      {GROUPS.map((group) => (
        <fieldset key={group.titleKey}>
          <legend>{t(group.titleKey)}</legend>
          {group.titleKey === 'theme.colours' && resolved.scheme === 'auto' ? (
            <p className="field-note">{t('theme.shadesHelp')}</p>
          ) : null}
          {group.keys.map((key) => (
            <TokenField key={key} settingKey={key} draft={draft} resolved={resolved} onSet={onSet} />
          ))}
        </fieldset>
      ))}
    </details>
  );
};
