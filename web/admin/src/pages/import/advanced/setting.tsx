/**
 * One Advanced setting: its name, one plain line that says what it does, and
 * its control. Every setting has the line, so a precise word on its name is
 * always explained where it is read.
 */
import { useId } from 'react';
import type { ReactElement, ReactNode } from 'react';

export interface SettingProps {
  readonly label: string;
  readonly plain: ReactNode;
  /** Receives the id of the plain line, for the control's `aria-describedby`. */
  readonly children: (described: string) => ReactNode;
}

export const Setting = ({ label, plain, children }: SettingProps): ReactElement => {
  const id = useId();
  return (
    <div className="adv-setting" role="group" aria-labelledby={`${id}-name`}>
      <div className="adv-setting-label">
        <span id={`${id}-name`} className="adv-setting-name">
          {label}
        </span>
        <p id={`${id}-plain`} className="adv-plain">
          {plain}
        </p>
      </div>
      <div className="adv-setting-control">{children(`${id}-plain`)}</div>
    </div>
  );
};

