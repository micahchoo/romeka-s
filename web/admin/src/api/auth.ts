/**
 * The API key: where it is validated, where it is kept, and who may read it.
 *
 * The API's key adapter reads two query parameters, `key_identity` and
 * `key_credential` (`crates/omeka-api/src/auth.rs`), and both are 32 characters
 * from the alphabet `ApiKey::STRING_CHARLIST` (alphanumerics). A key arrives as
 * external input, typed by a user or read back from `localStorage`, so this
 * module is the boundary that parses it. Everything downstream takes `ApiKey`
 * and trusts it.
 *
 * The key lives in `localStorage` so a reload keeps the session. It is never
 * logged and never rendered; the signed-in surface shows the identity, which is
 * the public half of the pair.
 */
import { isRecord } from './parse';

/** `MvcListeners::authenticateApiKey` query parameter names. */
export const KEY_IDENTITY_PARAM = 'key_identity';
export const KEY_CREDENTIAL_PARAM = 'key_credential';

/** `ApiKey::STRING_LENGTH`. */
const KEY_LENGTH = 32;

/** `ApiKey::STRING_CHARLIST`. */
const KEY_CHARS = /^[A-Za-z0-9]+$/;

/** The `localStorage` key. Namespaced, because the origin may host other apps. */
export const KEY_STORAGE = 'romeka.apiKey';

/** A validated key pair. */
export interface ApiKey {
  readonly identity: string;
  readonly credential: string;
}

/** One message per field the form must correct. */
export interface KeyErrors {
  readonly identity?: string;
  readonly credential?: string;
}

export type KeyParse =
  | { readonly ok: true; readonly key: ApiKey }
  | { readonly ok: false; readonly errors: KeyErrors };

/** True when a string could be an identity or a credential upstream stored. */
const isKeyPartWellFormed = (value: string): boolean =>
  value.length === KEY_LENGTH && KEY_CHARS.test(value);

const shapeError = (label: string, value: string): string =>
  value === ''
    ? `Enter the 32-character key ${label}.`
    : `A key ${label} is 32 letters or digits. This is ${String(value.length)}.`;

/**
 * Validate both halves of a pair. A pair is used only when both are well formed,
 * so the caller cannot send one parameter without the other.
 */
export const parseApiKey = (identity: string, credential: string): KeyParse => {
  const errors: { identity?: string; credential?: string } = {};
  if (!isKeyPartWellFormed(identity)) {
    errors.identity = shapeError('identity', identity);
  }
  if (!isKeyPartWellFormed(credential)) {
    errors.credential = shapeError('credential', credential);
  }
  if (errors.identity !== undefined || errors.credential !== undefined) {
    return { ok: false, errors };
  }
  return { ok: true, key: { identity, credential } };
};

const readStored = (): ApiKey | null => {
  const raw = window.localStorage.getItem(KEY_STORAGE);
  if (raw === null) {
    return null;
  }
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!isRecord(value)) {
    return null;
  }
  const identity = value['identity'];
  const credential = value['credential'];
  if (typeof identity !== 'string' || typeof credential !== 'string') {
    return null;
  }
  const parsed = parseApiKey(identity, credential);
  return parsed.ok ? parsed.key : null;
};

type Listener = () => void;

const listeners = new Set<Listener>();
let current: ApiKey | null = null;
let loaded = false;

const publish = (key: ApiKey | null): void => {
  current = key;
  for (const listener of listeners) {
    listener();
  }
};

/**
 * The key every request should carry, or `null` when signed out. The first call
 * reads `localStorage`, so a signed-in session survives a reload.
 */
export const currentKey = (): ApiKey | null => {
  if (!loaded) {
    loaded = true;
    current = readStored();
  }
  return current;
};

/** Subscribe to sign-in and sign-out, for `useSyncExternalStore`. */
export const subscribeKey = (listener: Listener): (() => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};

export const signIn = (key: ApiKey): void => {
  window.localStorage.setItem(KEY_STORAGE, JSON.stringify(key));
  loaded = true;
  publish(key);
};

export const signOut = (): void => {
  window.localStorage.removeItem(KEY_STORAGE);
  loaded = true;
  publish(null);
};
