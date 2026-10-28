/**
 * A list screen's place for the notice of a record of its kind that a page just moved to the
 * trash. It takes the slot once, when the screen mounts, and loads the notice's chunk only when
 * the slot held such a record.
 */
import type { ReactElement } from 'react';
import { Suspense, lazy, useEffect, useState } from 'react';
import type { TrashOf } from '../../api/bulk';
import { forgetTrashed, peekTrashed } from './slot';

const TrashedNotice = lazy(() => import('./notice'));

export const TrashedHere = ({
  of,
  onRestored,
}: {
  readonly of: TrashOf;
  /** The record is back: the screen reads its list again. */
  readonly onRestored: () => void;
}): ReactElement | null => {
  const [trashed] = useState(() => peekTrashed(of));
  useEffect(() => {
    if (trashed !== null) {
      forgetTrashed(trashed);
    }
  }, [trashed]);
  return trashed === null ? null : (
    <Suspense fallback={null}>
      <TrashedNotice trashed={trashed} onRestored={onRestored} />
    </Suspense>
  );
};
