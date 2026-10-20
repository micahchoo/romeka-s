/**
 * The `color` field control: a colour picker beside the hex code it stores,
 * and, for a field that offers them, the theme's colour roles as swatches.
 *
 * The renderer accepts only `#rgb` or `#rrggbb` (`layout.rs::is_hex_color`)
 * and drops anything else, so a code it would drop is marked here instead. A
 * theme role follows the Theme page, so it is the choice the help points to;
 * the swatches show the site's own colours once its palette has loaded.
 */
import { useEffect, useState } from 'react';
import type { CSSProperties, ReactElement } from 'react';
import type { JsonValue } from '../../../../api/types';
import type { ColorToken } from '../../blockSchema';
import { useEditorText } from '../messages';
import { Help, helpId } from '../Labelled';
import './controls.css';

const HEX = /^#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/;

/** The `#rrggbb` a colour input needs, from a stored `#rgb` or `#rrggbb`. */
export const pickerValue = (value: string): string => {
  if (!HEX.test(value)) {
    return '#ffffff';
  }
  if (value.length === 7) {
    return value.toLowerCase();
  }
  const [, r = '', g = '', b = ''] = value.toLowerCase();
  return `#${r}${r}${g}${g}${b}${b}`;
};

/** The site's role colours, loaded with the theme resolver the first time a swatch row shows. */
const usePalette = (
  themeSettings: JsonValue | null | undefined,
  wanted: boolean,
): ReadonlyMap<string, string> | null => {
  const [palette, setPalette] = useState<ReadonlyMap<string, string> | null>(null);
  useEffect(() => {
    if (!wanted || themeSettings === undefined) {
      return;
    }
    let live = true;
    void import('./sitePalette').then((module) => {
      if (live) {
        setPalette(module.sitePalette(themeSettings));
      }
    });
    return () => {
      live = false;
    };
  }, [themeSettings, wanted]);
  return palette;
};

/** A loaded colour fills the swatch; until then the sheet's empty-swatch stripe shows. */
const swatchStyle = (colour: string | undefined): CSSProperties | undefined =>
  colour === undefined ? undefined : { background: colour };

export interface ColorFieldProps {
  readonly id: string;
  readonly label: string;
  readonly help: string;
  /** The stored hex colour, or the empty string. */
  readonly value: string;
  readonly onChange: (value: string) => void;
  /** The theme roles to offer; empty for a field that takes hex only. */
  readonly tokens?: readonly ColorToken[];
  /** The stored role, or the empty string. */
  readonly token?: string;
  readonly onToken?: (token: string) => void;
  /** The site's theme settings, which colour the swatches. */
  readonly themeSettings?: JsonValue | null;
}

export const ColorField = ({
  id,
  label,
  help,
  value,
  onChange,
  tokens = [],
  token = '',
  onToken,
  themeSettings,
}: ColorFieldProps): ReactElement => {
  const t = useEditorText();
  const palette = usePalette(themeSettings, tokens.length > 0);
  const invalid = value !== '' && !HEX.test(value);
  const chosen = tokens.find((entry) => entry.value === token);
  return (
    <div className="block-field">
      <label htmlFor={id}>{label}</label>
      {tokens.length === 0 || onToken === undefined ? null : (
        <div className="color-tokens" role="radiogroup" aria-label={t('color.tokens', { label })}>
          {tokens.map((entry) => (
            <button
              key={entry.value}
              type="button"
              role="radio"
              aria-checked={entry.value === token}
              className="color-token"
              title={t(entry.label)}
              aria-label={t(entry.label)}
              onClick={() => {
                onToken(entry.value === token ? '' : entry.value);
              }}
            >
              <span
                className="color-token-swatch"
                style={swatchStyle(palette?.get(entry.value))}
                aria-hidden="true"
              />
            </button>
          ))}
        </div>
      )}
      {chosen === undefined ? null : (
        <p className="field-note">{t('color.tokenChosen', { name: t(chosen.label) })}</p>
      )}
      <div className="color-field">
        <input
          type="color"
          aria-label={t('color.picker')}
          value={pickerValue(value)}
          onChange={(event) => {
            onChange(event.target.value);
          }}
        />
        <input
          id={id}
          type="text"
          value={value}
          placeholder="#rrggbb"
          spellCheck={false}
          aria-invalid={invalid}
          aria-describedby={helpId(id)}
          onChange={(event) => {
            onChange(event.target.value.trim());
          }}
        />
        <button
          type="button"
          disabled={value === '' && token === ''}
          onClick={() => {
            onChange('');
          }}
        >
          {t('color.clear')}
        </button>
      </div>
      <Help id={id} text={help} />
      {invalid ? (
        <p className="field-error" role="alert">
          {t('color.invalid')}
        </p>
      ) : null}
    </div>
  );
};
