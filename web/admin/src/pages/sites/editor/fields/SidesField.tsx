/**
 * The `sides` field control: one CSS length per side, stored under four keys
 * (`padding_top` and the rest). Linked, one input writes all four; unlinked,
 * each side has its own. A preset writes all four at once.
 *
 * The control starts linked when the four stored values agree, so a block
 * styled side by side opens on the four inputs that show it.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import type { JsonValue } from '../../../../api/types';
import { useEditorText } from '../messages';
import type { EditorMessageKey } from '../messages';
import type { SideKeys } from '../../blockSchema';
import { Help, helpId } from '../Labelled';
import './controls.css';

type Side = keyof SideKeys;

const SIDES: readonly { readonly side: Side; readonly label: EditorMessageKey }[] = [
  { side: 'top', label: 'blockChoice.top' },
  { side: 'right', label: 'blockChoice.right' },
  { side: 'bottom', label: 'blockChoice.bottom' },
  { side: 'left', label: 'blockChoice.left' },
];

/** The presets, in the order the buttons show them, and the length each writes. */
export const SIDE_PRESETS: readonly { readonly label: EditorMessageKey; readonly value: string }[] = [
  { label: 'sides.none', value: '0' },
  { label: 'sides.small', value: '0.5rem' },
  { label: 'sides.medium', value: '1rem' },
  { label: 'sides.large', value: '2rem' },
];

const textOf = (value: JsonValue | undefined): string =>
  typeof value === 'string' ? value : typeof value === 'number' ? String(value) : '';

export interface SidesFieldProps {
  readonly id: string;
  readonly label: string;
  readonly help: string;
  readonly keys: SideKeys;
  readonly record: Readonly<Record<string, JsonValue>>;
  /** Writes several stored keys as one edit. */
  readonly onPatch: (values: Readonly<Record<string, JsonValue>>) => void;
}

export const SidesField = ({ id, label, help, keys, record, onPatch }: SidesFieldProps): ReactElement => {
  const t = useEditorText();
  const values = SIDES.map(({ side }) => textOf(record[keys[side]]));
  const same = values.every((value) => value === values[0]);
  const [linked, setLinked] = useState(same);
  const setAll = (value: string): void => {
    onPatch(Object.fromEntries(SIDES.map(({ side }) => [keys[side], value])));
  };
  return (
    <fieldset className="block-field sides-field" aria-describedby={helpId(id)}>
      <legend>{label}</legend>
      <div className="sides-presets" role="group" aria-label={t('sides.presets')}>
        {SIDE_PRESETS.map((preset) => (
          <button
            key={preset.value}
            type="button"
            aria-pressed={same && values[0] === preset.value}
            onClick={() => {
              setLinked(true);
              setAll(preset.value);
            }}
          >
            {t(preset.label)}
          </button>
        ))}
      </div>
      <label className="block-check" htmlFor={`${id}-linked`}>
        <input
          id={`${id}-linked`}
          type="checkbox"
          checked={linked}
          onChange={(event) => {
            setLinked(event.target.checked);
            if (event.target.checked) {
              setAll(values[0] ?? '');
            }
          }}
        />
        {t('sides.linked')}
      </label>
      {linked ? (
        <div className="sides-inputs sides-inputs--linked">
          <label htmlFor={id}>{t('sides.all')}</label>
          <input
            id={id}
            type="text"
            inputMode="decimal"
            value={values[0] ?? ''}
            onChange={(event) => {
              setAll(event.target.value);
            }}
          />
        </div>
      ) : (
        <div className="sides-inputs">
          {SIDES.map(({ side, label: sideLabel }, index) => (
            <div key={side}>
              <label htmlFor={`${id}-${side}`}>{t(sideLabel)}</label>
              <input
                id={`${id}-${side}`}
                type="text"
                inputMode="decimal"
                value={values[index] ?? ''}
                onChange={(event) => {
                  onPatch({ [keys[side]]: event.target.value });
                }}
              />
            </div>
          ))}
        </div>
      )}
      <Help id={id} text={help} />
    </fieldset>
  );
};
