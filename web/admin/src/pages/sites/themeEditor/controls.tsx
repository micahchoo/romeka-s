/**
 * The Theme page's controls: a colour input, and a token row that carries a
 * label, the control, one help line and a Reset button once the curator has
 * changed the preset's value.
 */
import type { ReactElement, ReactNode } from 'react';
import { isThemeMessageKey, useThemeText } from './messages';
import type { ThemeMessageKey } from './messages';
import { isHexColour, normaliseHex } from './colour';
import type { ThemeSetting } from './registry';
import type { TokenValue } from './draft';

const camel = (value: string): string => value.replace(/-([a-z])/g, (_, ch: string) => ch.toUpperCase());

/** The catalogue key for a choice, `themeChoice.<camelCase>`, when the catalogue has one. */
export const choiceKey = (choice: string): ThemeMessageKey | null => {
  const key = `themeChoice.${camel(choice)}`;
  return isThemeMessageKey(key) ? key : null;
};

/** A hex colour: a native picker and a text field, so a curator can paste a value. */
export const ColourInput = ({
  id,
  label,
  value,
  fallback,
  textLabel,
  onChange,
}: {
  readonly id: string;
  readonly label: string;
  readonly value: string;
  readonly fallback: string;
  /** The text field's own name, when a `<label for>` does not give it the right one. */
  readonly textLabel?: string;
  readonly onChange: (value: string) => void;
}): ReactElement => {
  const t = useThemeText();
  const invalid = !isHexColour(value);
  return (
    <span className="theme-colour-input">
      <input
        type="color"
        aria-label={label}
        value={normaliseHex(value) ?? fallback}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      />
      <input
        id={id}
        type="text"
        aria-label={textLabel}
        spellCheck={false}
        value={value}
        aria-invalid={invalid}
        onChange={(event) => {
          onChange(event.target.value.trim());
        }}
      />
      {invalid ? (
        <span className="field-note" role="alert">
          {t('theme.hexHint')}
        </span>
      ) : null}
    </span>
  );
};

/** The control one token renders, by kind. */
export const TokenControl = ({
  setting,
  value,
  onChange,
}: {
  readonly setting: ThemeSetting;
  readonly value: TokenValue;
  readonly onChange: (value: TokenValue) => void;
}): ReactElement => {
  const t = useThemeText();
  const id = `theme-setting-${setting.key}`;
  const label = t(setting.labelKey);
  switch (setting.kind) {
    case 'colour':
      return (
        <ColourInput
          id={id}
          label={label}
          value={String(value)}
          fallback="#000000"
          onChange={onChange}
        />
      );
    case 'font':
    case 'choice':
      return (
        <select
          id={id}
          value={String(value)}
          onChange={(event) => {
            onChange(event.target.value);
          }}
        >
          {setting.choices.map((choice) => {
            const key = choiceKey(choice);
            return (
              <option key={choice} value={choice}>
                {key === null ? choice : t(key)}
              </option>
            );
          })}
        </select>
      );
    case 'number':
      return (
        <span className="theme-number">
          <input
            type="range"
            aria-label={label}
            min={setting.min}
            max={setting.max}
            step={setting.step}
            value={Number(value)}
            onChange={(event) => {
              onChange(Number(event.target.value));
            }}
          />
          <input
            id={id}
            type="number"
            min={setting.min}
            max={setting.max}
            step={setting.step}
            value={Number(value)}
            onChange={(event) => {
              onChange(Number(event.target.value));
            }}
          />
        </span>
      );
    default:
      return <span />;
  }
};

/** One labelled row: the control, its help line, and Reset when the value is the curator's own. */
export const TokenRow = ({
  setting,
  changed,
  onReset,
  children,
}: {
  readonly setting: ThemeSetting;
  readonly changed: boolean;
  readonly onReset: () => void;
  readonly children: ReactNode;
}): ReactElement => {
  const t = useThemeText();
  const id = `theme-setting-${setting.key}`;
  const label = t(setting.labelKey);
  return (
    <div className="theme-row" data-changed={changed}>
      <label htmlFor={id}>{label}</label>
      <div className="theme-row-control">
        {children}
        {changed ? (
          <button
            type="button"
            className="theme-reset"
            aria-label={t('theme.resetLabel', { label })}
            onClick={onReset}
          >
            {t('theme.reset')}
          </button>
        ) : null}
      </div>
      <p className="field-note theme-help">{t(setting.helpKey)}</p>
    </div>
  );
};
