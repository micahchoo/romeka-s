import type { ReactElement, ReactNode } from 'react';

export interface TableScrollProps {
  /** The region's accessible name, so a screen reader announces what scrolls. */
  readonly label: string;
  readonly children: ReactNode;
}

/**
 * A horizontally scrollable table wrapper. A table wider than the column
 * scrolls, so the wrapper takes focus and names itself as a region: a keyboard
 * user can reach and scroll it, and a screen reader announces what it holds.
 * Without both, axe reports `scrollable-region-focusable`.
 */
export const TableScroll = ({ label, children }: TableScrollProps): ReactElement => (
  <div className="table-scroll" role="region" aria-label={label} tabIndex={0}>
    {children}
  </div>
);
