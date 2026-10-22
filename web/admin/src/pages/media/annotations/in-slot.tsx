import type { ReactElement, ReactNode } from 'react';
import { createPortal } from 'react-dom';

/**
 * Its children drawn into a place on the stage, or where they stand when the
 * page gave the panel no stage (the item preview, a test).
 */
export const InSlot = ({
  into,
  children,
}: {
  readonly into: HTMLElement | null;
  readonly children: ReactNode;
}): ReactElement => (into === null ? <>{children}</> : createPortal(children, into));
