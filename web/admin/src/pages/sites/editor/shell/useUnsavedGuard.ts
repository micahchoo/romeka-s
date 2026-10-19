/**
 * The unsaved-changes guard. `beforeunload` covers a reload or a closed tab and
 * `popstate` covers a route change, since the SPA's routes live in the
 * fragment and a fragment change cannot be cancelled. It listens to `popstate`,
 * not `hashchange`: React handles `hashchange` at discrete priority, so the
 * shell re-renders the new route and unmounts this guard before a later
 * `hashchange` listener runs. A declined route change puts the old hash back,
 * which fires a second event the guard ignores.
 */
import { useEffect, useRef } from 'react';

export interface UnsavedGuard {
  /** Stop guarding, for a save that navigates on purpose. */
  readonly allowLeave: () => void;
}

export const useUnsavedGuard = (dirty: boolean, message: string): UnsavedGuard => {
  const bypass = useRef(false);
  const approved = useRef(window.location.hash);

  useEffect(() => {
    if (!dirty) {
      return;
    }
    approved.current = window.location.hash;
    const onBeforeUnload = (event: BeforeUnloadEvent): void => {
      // A cancelled beforeunload event is what makes the browser ask; the
      // deprecated `returnValue` is unnecessary for that and rejected by lint.
      event.preventDefault();
    };
    const onHashChange = (): void => {
      if (bypass.current) {
        return;
      }
      const next = window.location.hash;
      if (next === approved.current) {
        return;
      }
      if (window.confirm(message)) {
        approved.current = next;
        return;
      }
      window.location.hash = approved.current;
    };
    window.addEventListener('beforeunload', onBeforeUnload);
    window.addEventListener('popstate', onHashChange);
    return () => {
      window.removeEventListener('beforeunload', onBeforeUnload);
      window.removeEventListener('popstate', onHashChange);
    };
  }, [dirty, message]);

  return {
    allowLeave: () => {
      bypass.current = true;
    },
  };
};
