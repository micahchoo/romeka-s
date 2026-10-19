/**
 * Which lessons the signed-in user has seen, and whether they want lessons.
 *
 * The state lives on the server, in two of the user's settings
 * (`romeka_lessons_seen`, `romeka_hide_lessons`), so it follows the user to
 * any browser. It arrives with the session (`GET /api/me`, a sign-in), and a
 * lesson marked seen here is written back at once.
 *
 * With nobody signed in there is no user to store the state for, so no lesson
 * shows. A lesson that could never stay dismissed would repeat on every visit.
 */
import { api } from '../../api/client';
import { currentUser, subscribeSession } from '../../api/session';
import type { User, UserId } from '../../api/types';
import type { WrittenLessonId } from './registry';

/** What a lesson's anchor shows: nothing, the lesson open, or its "?". */
export type LessonView = 'off' | 'fresh' | 'seen';

interface Known {
  readonly user: UserId;
  readonly hidden: boolean;
  readonly seen: ReadonlySet<string>;
}

const fromUser = (user: User | null, before: Known | null): Known | null => {
  if (user === null) {
    return null;
  }
  // The session answer can be older than a write made since, so seen ids only grow.
  const kept = before?.user === user.id ? before.seen : [];
  return {
    user: user.id,
    hidden: user.lessons.hidden,
    seen: new Set([...kept, ...user.lessons.seen]),
  };
};

let known = fromUser(currentUser(), null);
const listeners = new Set<() => void>();

const publish = (next: Known | null): void => {
  known = next;
  for (const listener of listeners) {
    listener();
  }
};

subscribeSession(() => {
  publish(fromUser(currentUser(), known));
});

/** Subscribe to every change, for `useSyncExternalStore`. */
export const subscribeLessons = (listener: () => void): (() => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};

export const lessonView = (id: WrittenLessonId): LessonView => {
  if (known === null || known.hidden) {
    return 'off';
  }
  return known.seen.has(id) ? 'seen' : 'fresh';
};

// One write at a time, each sending the whole set as it stands when it leaves,
// so two lessons marked together cannot land out of order and drop one.
let writes = Promise.resolve();

/** Record that the lesson has shown, here and on the server. */
export const markSeen = (id: WrittenLessonId): void => {
  if (known === null || known.hidden || known.seen.has(id)) {
    return;
  }
  const user = known.user;
  publish({ ...known, seen: new Set([...known.seen, id]) });
  writes = writes
    .then(async () => {
      if (known?.user === user) {
        await api.updateUserSettings(user, { lessonsSeen: [...known.seen] });
      }
    })
    .catch((error: unknown) => {
      console.error(error);
    });
};

/** The user's own "Show lessons" switch was saved: follow it without a reload. */
export const followLessonsSwitch = (user: UserId, hidden: boolean): void => {
  if (known?.user === user) {
    publish({ ...known, hidden });
  }
};
