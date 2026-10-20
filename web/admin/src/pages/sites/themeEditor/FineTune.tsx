/**
 * The Fine-tune disclosure: every single token, grouped as colours, type,
 * layout and page structure, each with its help line and a Reset to the
 * preset's value.
 */
import type { ReactElement } from 'react';
import { useThemeText } from './messages';
import type { ThemeMessageKey } from './messages';
import { settingFor } from './registry';
import type { ThemeSetting } from './registry';
import type { Resolved, ThemeDraft, TokenValue } from './draft';
import { THEME_REGISTRY } from './registry';
import { TokenControl, TokenRow } from './controls';

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
          {group.keys.map((key) => {
            const setting = settingFor(key);
            if (setting === undefined) {
              return null;
            }
            return (
              <TokenRow
                key={key}
                setting={setting}
                changed={draft.overrides[key] !== undefined}
                onReset={() => {
                  onSet(key, null);
                }}
              >
                <TokenControl
                  setting={setting}
                  value={shownValue(setting, draft, resolved)}
                  onChange={(value) => {
                    onSet(key, value);
                  }}
                />
              </TokenRow>
            );
          })}
        </fieldset>
      ))}
    </details>
  );
};
