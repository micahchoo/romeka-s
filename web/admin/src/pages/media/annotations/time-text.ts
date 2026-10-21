/**
 * A time as an editor types it and as the panel shows it.
 *
 * The same value has four lengths: `62`, `1:02`, `1:02.5` and `1:02:03` all
 * name a moment in the recording. The server stores seconds with at most three
 * decimals (contract 6.3), so the parser keeps milliseconds and the formatter
 * rounds to one decimal, which is what an editor can hear. The notes list and
 * the time editor both read these two functions, so a range never reads one way
 * in the list and another in the fields.
 *
 * Parsing is strict on purpose: a minutes or hours field, when there is a
 * field after it, holds a whole count, and a seconds field after another field
 * stays under 60, so `1:75` is refused rather than read as `2:15`.
 */

/** A range on the recording, in milliseconds. */
export interface TimeRange {
  readonly startMs: number;
  readonly endMs: number;
}

/** Why a range cannot be saved, or `null` when it can. */
export type TimeProblem = 'order' | 'pastEnd';

const SECONDS = /^\d+(?:\.\d{1,3})?$/;
const WHOLE = /^\d+$/;

/** Milliseconds for a typed time, or `null` when it is not one. */
export const parseClock = (text: string): number | null => {
  const parts = text.trim().split(':');
  if (parts.length === 0 || parts.length > 3) {
    return null;
  }
  const last = parts[parts.length - 1] ?? '';
  if (!SECONDS.test(last)) {
    return null;
  }
  let milliseconds = 0;
  let scale = 1;
  for (let index = parts.length - 1; index >= 0; index -= 1) {
    const part = parts[index] ?? '';
    if (index !== parts.length - 1 && !WHOLE.test(part)) {
      return null;
    }
    const value = Number(part);
    const upper = index === parts.length - 1 && parts.length > 1 ? 60 : null;
    if (!Number.isFinite(value) || (upper !== null && value >= upper)) {
      return null;
    }
    milliseconds += value * scale;
    scale *= 60;
  }
  return Math.round(milliseconds * 1000);
};

/** A time as `1:02.5`, `1:02`, `1:02:03` or `1:02:03.5`, in one decimal. */
export const clockText = (milliseconds: number): string => {
  const tenths = Math.round(Math.max(0, milliseconds) / 100);
  const hours = Math.floor(tenths / 36_000);
  const minutes = Math.floor((tenths % 36_000) / 600);
  const seconds = (tenths % 600) / 10;
  const whole = Math.floor(seconds);
  const fraction = Math.round((seconds - whole) * 10);
  const secondText = `${String(whole).padStart(2, '0')}${fraction === 0 ? '' : `.${String(fraction)}`}`;
  return hours === 0
    ? `${String(minutes)}:${secondText}`
    : `${String(hours)}:${String(minutes).padStart(2, '0')}:${secondText}`;
};

/**
 * The five seconds after a position, which is what **Note from here** marks
 * and what the next note starts from once one is saved, capped at the length
 * when the server knows it.
 */
export const defaultRange = (startMs: number, durationMs: number | null): TimeRange => {
  const start = Math.max(0, Math.round(startMs));
  const end = start + 5000;
  return { startMs: start, endMs: durationMs === null ? end : Math.min(end, durationMs) };
};

/** Whether a range may be saved: it needs a length, and cannot pass the end. */
export const rangeProblem = (
  startMs: number,
  endMs: number,
  durationMs: number | null,
): TimeProblem | null => {
  if (!Number.isFinite(startMs) || !Number.isFinite(endMs) || startMs < 0 || endMs <= startMs) {
    return 'order';
  }
  return durationMs !== null && endMs > durationMs ? 'pastEnd' : null;
};
