/**
 * The four things the panel does to the stage's player: read its position,
 * seek it, start it and pause it.
 *
 * The element belongs to the stage, so the notes panel never renders it. Both
 * the time editor and the list's entry click reach it through these functions,
 * which is why a note seeks the player whether the editor clicked its segment
 * or its row. A browser that refuses a call costs the click, never the note.
 */

/** The player's position in milliseconds. */
export const positionOf = (player: HTMLMediaElement | null): number =>
  player === null ? 0 : Math.max(0, Math.round(player.currentTime * 1000));

export const seek = (player: HTMLMediaElement | null, milliseconds: number): void => {
  if (player === null) {
    return;
  }
  try {
    player.currentTime = milliseconds / 1000;
  } catch {
    // A file the browser cannot seek still leaves the note readable.
  }
};

export const pause = (player: HTMLMediaElement | null): void => {
  try {
    player?.pause();
  } catch {
    // The player was never playing.
  }
};

export const play = (player: HTMLMediaElement | null): void => {
  if (player === null) {
    return;
  }
  try {
    void player.play().catch(() => {
      // Autoplay was refused; the player's own controls remain.
    });
  } catch {
    // Same.
  }
};
