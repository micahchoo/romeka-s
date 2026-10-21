/**
 * Where a media's player stopped, for this tab.
 *
 * The stage keeps one position per media in `sessionStorage`, so an editor who
 * follows a link to the notes and comes back starts where they stopped, and a
 * second media's position never leaks into the first. Storage can be refused
 * (private mode, a full quota), which costs the position and never the page.
 */
const PREFIX = 'romeka.media.position.';

const keyOf = (mediaId: number): string => `${PREFIX}${String(mediaId)}`;

/** Keep the position, rounded to a tenth of a second. */
export const rememberPosition = (mediaId: number, seconds: number): void => {
  if (!Number.isFinite(seconds) || seconds < 0) {
    return;
  }
  try {
    window.sessionStorage.setItem(keyOf(mediaId), String(Math.round(seconds * 10) / 10));
  } catch {
    // The store refused the write. The player works; only the resume is lost.
  }
};

/** The kept position, or `null` when there is none or it cannot be read. */
export const recallPosition = (mediaId: number): number | null => {
  try {
    const stored = window.sessionStorage.getItem(keyOf(mediaId));
    if (stored === null) {
      return null;
    }
    const seconds = Number(stored);
    return Number.isFinite(seconds) && seconds >= 0 ? seconds : null;
  } catch {
    return null;
  }
};
