/**
 * Which lesson is open now. At most one lesson opens on its own at a time: a
 * page where two moments come together would otherwise stack two notes, and a
 * beginner reads neither.
 *
 * Every anchor whose lesson is waiting to open joins the queue. When nothing
 * is open, the queue opens the waiting lesson lowest on the ladder (`RUNGS`),
 * then the one first in `LESSONS`, then the one that joined first. The choice
 * waits one microtask, so anchors that mount in one render all take part.
 * "Got it" releases the open lesson and the next one opens; an anchor that
 * leaves the page releases it too. A waiting lesson is not marked seen, so if
 * the user leaves first it opens on the next visit.
 */
import { LESSONS, RUNGS } from './registry';
import type { LessonId } from './registry';

/** One `useLesson` call in place, named by its own `useId`. */
export type AnchorKey = string;

interface Waiting {
  readonly lesson: LessonId;
  readonly joined: number;
}

const ORDER = Object.keys(LESSONS);

const compare = (a: Waiting, b: Waiting): number =>
  RUNGS.indexOf(LESSONS[a.lesson].rung) - RUNGS.indexOf(LESSONS[b.lesson].rung) ||
  ORDER.indexOf(a.lesson) - ORDER.indexOf(b.lesson) ||
  a.joined - b.joined;

const waiting = new Map<AnchorKey, Waiting>();
let open: AnchorKey | null = null;
let joined = 0;
let settling = false;
const listeners = new Set<() => void>();

const notify = (): void => {
  for (const listener of listeners) {
    listener();
  }
};

const settle = (): void => {
  settling = false;
  if (open !== null) {
    return;
  }
  let next: [AnchorKey, Waiting] | null = null;
  for (const entry of waiting) {
    if (next === null || compare(entry[1], next[1]) < 0) {
      next = entry;
    }
  }
  if (next !== null) {
    open = next[0];
    notify();
  }
};

const scheduleSettle = (): void => {
  if (!settling) {
    settling = true;
    queueMicrotask(settle);
  }
};

/** Subscribe to every change, for `useSyncExternalStore`. */
export const subscribeQueue = (listener: () => void): (() => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};

/** The anchor whose lesson is open now, if any. */
export const openAnchor = (): AnchorKey | null => open;

/** The user is done with the open lesson: the next waiting one may open. */
export const release = (anchor: AnchorKey): void => {
  waiting.delete(anchor);
  if (open === anchor) {
    open = null;
    notify();
    scheduleSettle();
  }
};

/** The anchor's lesson waits to open; the returned function withdraws it. */
export const join = (anchor: AnchorKey, lesson: LessonId): (() => void) => {
  joined += 1;
  waiting.set(anchor, { lesson, joined });
  scheduleSettle();
  return () => {
    release(anchor);
  };
};
