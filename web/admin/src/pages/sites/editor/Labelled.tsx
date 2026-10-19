/**
 * The one label and control layout of the page editor: the label above its
 * control, an error under it. The page settings, the inspector and every field
 * control render through these, so the two panels cannot drift apart. The
 * layout itself is `.block-field` in `src/styles.css`.
 */
import type { ReactElement, ReactNode } from 'react';

export const Labelled = ({
  id,
  label,
  error = null,
  children,
}: {
  readonly id: string;
  readonly label: string;
  readonly error?: ReactNode;
  readonly children: ReactNode;
}): ReactElement => (
  <div className="block-field">
    <label htmlFor={id}>{label}</label>
    {children}
    {error}
  </div>
);

/** A checkbox and its label on one line, inside the same field row. */
export const Checked = ({
  id,
  label,
  checked,
  onChange,
}: {
  readonly id: string;
  readonly label: string;
  readonly checked: boolean;
  readonly onChange: (checked: boolean) => void;
}): ReactElement => (
  <div className="block-field">
    <label className="block-check" htmlFor={id}>
      <input
        id={id}
        type="checkbox"
        checked={checked}
        onChange={(event) => {
          onChange(event.target.checked);
        }}
      />
      {label}
    </label>
  </div>
);
