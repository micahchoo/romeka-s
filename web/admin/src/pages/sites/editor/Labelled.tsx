/**
 * The one label and control layout of the page editor: the label above its
 * control, then a help line and an error under it. The page settings, the inspector and every field
 * control render through these, so the two panels cannot drift apart. The
 * layout itself is `.block-field` in `src/styles.css`.
 */
import type { ReactElement, ReactNode } from 'react';

/** The id of a control's help line, which the control names in `aria-describedby`. */
export const helpId = (id: string): string => `${id}-help`;

/** The one line under a control that says what the visitor sees. */
export const Help = ({ id, text }: { readonly id: string; readonly text: string }): ReactElement => (
  <p id={helpId(id)} className="field-help">
    {text}
  </p>
);

export const Labelled = ({
  id,
  label,
  help,
  error = null,
  children,
}: {
  readonly id: string;
  readonly label: string;
  /** The help line; the control names it with `aria-describedby={helpId(id)}`. */
  readonly help?: string;
  readonly error?: ReactNode;
  readonly children: ReactNode;
}): ReactElement => (
  <div className="block-field">
    <label htmlFor={id}>{label}</label>
    {children}
    {help === undefined ? null : <Help id={id} text={help} />}
    {error}
  </div>
);

/** A checkbox and its label on one line, inside the same field row. */
export const Checked = ({
  id,
  label,
  help,
  checked,
  onChange,
}: {
  readonly id: string;
  readonly label: string;
  readonly help?: string;
  readonly checked: boolean;
  readonly onChange: (checked: boolean) => void;
}): ReactElement => (
  <div className="block-field">
    <label className="block-check" htmlFor={id}>
      <input
        id={id}
        type="checkbox"
        checked={checked}
        {...(help === undefined ? {} : { 'aria-describedby': helpId(id) })}
        onChange={(event) => {
          onChange(event.target.checked);
        }}
      />
      {label}
    </label>
    {help === undefined ? null : <Help id={id} text={help} />}
  </div>
);
