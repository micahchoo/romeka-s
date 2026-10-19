/**
 * The `color` field control: a colour picker beside the hex code it stores.
 * The renderer accepts only `#rgb` or `#rrggbb` (`layout.rs::is_hex_color`) and
 * drops anything else, so a code it would drop is marked here instead.
 */
import type { ReactElement } from 'react';
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

export interface ColorFieldProps {
  readonly id: string;
  readonly label: string;
  readonly help: string;
  readonly value: string;
  readonly onChange: (value: string) => void;
}

export const ColorField = ({ id, label, help, value, onChange }: ColorFieldProps): ReactElement => {
  const t = useEditorText();
  const invalid = value !== '' && !HEX.test(value);
  return (
    <div className="block-field">
      <label htmlFor={id}>{label}</label>
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
          disabled={value === ''}
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
