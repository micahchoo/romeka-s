/**
 * "Delete this item", "Delete this media", "Delete item set": the button that opens the trash
 * confirm in place. Cancel closes it and gives focus back to the button. The confirm is its own
 * chunk, so a record page carries only this button.
 */
import type { ReactElement, ReactNode } from 'react';
import { Suspense, lazy, useEffect, useRef, useState } from 'react';
import type { TrashTarget } from './slot';

const TrashConfirm = lazy(() => import('./confirm'));

export interface TrashOpenerProps {
  readonly label: string;
  readonly target: TrashTarget;
  readonly name: string;
  /** Facts about the record that its own page counts, which the confirm shows. */
  readonly detail?: ReactNode;
}

export const TrashOpener = ({ label, target, name, detail }: TrashOpenerProps): ReactElement => {
  const [open, setOpen] = useState(false);
  const opener = useRef<HTMLButtonElement>(null);
  const closed = useRef(false);
  useEffect(() => {
    if (!open && closed.current) {
      opener.current?.focus();
    }
  }, [open]);
  if (open) {
    return (
      <Suspense fallback={<p role="status">…</p>}>
        <TrashConfirm
          target={target}
          name={name}
          detail={detail}
          onCancel={() => {
            closed.current = true;
            setOpen(false);
          }}
        />
      </Suspense>
    );
  }
  return (
    <section className="delete-control">
      <p>
        <button
          ref={opener}
          type="button"
          onClick={() => {
            setOpen(true);
          }}
        >
          {label}
        </button>
      </p>
    </section>
  );
};
