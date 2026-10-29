/**
 * The client session: who the server says is signed in.
 *
 * The session cookie is `httpOnly` and `SameSite=Lax`, so no script reads it and
 * no storage says whether one exists. The SPA learns the answer from
 * `GET /api/me` and keeps it here, so the header, the sign-out control and any
 * surface that asks who signed in read one value.
 *
 * The API key pair in `auth.ts` stays a second credential. The server resolves a
 * session cookie first and the key second ("Actor resolution" in the auth
 * contract), so a client may hold both and the session wins.
 */
import { parseLocale, setLocale } from '../i18n';
import { signOut } from './auth';
import type { User } from './types';

type Listener = () => void;

const listeners = new Set<Listener>();
let current: User | null = null;
/** Whether the server has answered who is signed in since the page loaded. */
let answered = false;

const publish = (user: User | null): void => {
  current = user;
  answered = true;
  for (const listener of listeners) {
    listener();
  }
};

/** The signed-in user, or `null` before a session is found and when there is none. */
export const currentUser = (): User | null => current;

/**
 * Whether the server has said who is signed in: `GET /api/me` answered, or a
 * sign-in or sign-out happened. Before that, `currentUser()` is `null` only
 * because nobody has asked yet.
 */
export const sessionAnswered = (): boolean => answered;

/** Subscribe to sign-in and sign-out, for `useSyncExternalStore`. */
export const subscribeSession = (listener: Listener): (() => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};

/**
 * Record what a sign-in or a `GET /api/me` answered. `null` is the server saying
 * nobody is signed in, which is what an anonymous `GET /api/me` returns.
 *
 * The admin locale the server resolved wins over the one this browser stored,
 * so a user's choice follows them; the stored one was only the first paint.
 * With no locale setting on the server, the stored choice stays.
 */
export const setSession = (user: User | null): void => {
  publish(user);
  if (user?.adminLocale != null) {
    setLocale(parseLocale(user.adminLocale)).catch((error: unknown) => {
      console.error(error);
    });
  }
};

/**
 * End the session on the client. The caller sends `POST /api/logout` first, so
 * the server has already revoked the session row; this drops the two client
 * halves, the key in `localStorage` and the user in memory.
 */
export const endSession = (): void => {
  signOut();
  publish(null);
};
