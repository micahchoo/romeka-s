/**
 * The sign-in surface: the person-facing form, the API key behind a disclosure,
 * and the header state that says who is signed in.
 *
 * Email and password is the primary path. It posts to `POST /api/login`, which
 * answers with the user and an `httpOnly` session cookie, so the client keeps the
 * user in `session.ts` and never the cookie. The key pair stays for programmatic
 * use, behind its own disclosure, and it validates both halves through the
 * `auth.ts` boundary before it stores anything.
 *
 * A refused request renders this same panel through `ErrorNotice`, which is how a
 * 403 becomes a way back in instead of a message with nowhere to go.
 */
import { useId, useState, useSyncExternalStore } from 'react';
import type { ReactElement, SyntheticEvent } from 'react';
import { api } from '../api/client';
import { currentKey, parseApiKey, signIn, subscribeKey } from '../api/auth';
import type { ApiKey, KeyErrors } from '../api/auth';
import { currentUser, endSession, setSession, subscribeSession } from '../api/session';
import type { User } from '../api/types';
import { useTranslator } from '../i18n';
import { formatError } from './format';
import { signInReturnPath } from './router';
import { useSave } from './save';

/** The stored key, tracked so a sign-in or sign-out re-renders the shell. */
const useApiKey = (): ApiKey | null =>
  useSyncExternalStore(subscribeKey, currentKey, currentKey);

/** The signed-in user, tracked for the same reason. */
export const useSession = (): User | null =>
  useSyncExternalStore(subscribeSession, currentUser, currentUser);

export interface SignInProps {
  /**
   * Runs after a credential is stored, so a caller that opened the panel can
   * close it. `undefined` is allowed so the panel can pass it through.
   */
  readonly onSignedIn?: (() => void) | undefined;
}

/**
 * The email and password form. An empty field is refused here; every other
 * answer comes from the server, including the 401 a wrong password gets and the
 * 429 the login limit gets, and each renders with its message and status.
 */
const PasswordSignIn = ({ onSignedIn }: SignInProps): ReactElement => {
  const translator = useTranslator();
  const { t } = translator;
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [formError, setFormError] = useState<string | null>(null);
  const { state: saveState, save } = useSave();
  const titleId = useId();

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (email.trim() === '' || password === '') {
      setFormError(t('auth.errorEmpty'));
      return;
    }
    setFormError(null);
    save(async () => {
      const user = await api.login(email, password);
      setSession(user);
      setEmail('');
      setPassword('');
      onSignedIn?.();
      const back = signInReturnPath(window.location.search);
      if (back !== null) {
        window.location.assign(back);
      }
    });
  };

  return (
    <form className="sign-in-form" aria-labelledby={titleId} onSubmit={submit}>
      <h2 className="sign-in-title" id={titleId}>
        {t('auth.panelHeading')}
      </h2>
      <p>
        <label htmlFor={`${titleId}-email`}>{t('auth.email')}</label>
        <input
          id={`${titleId}-email`}
          name="email"
          type="email"
          autoComplete="username"
          value={email}
          onChange={(event) => {
            setEmail(event.target.value);
          }}
        />
      </p>
      <p>
        <label htmlFor={`${titleId}-password`}>{t('auth.password')}</label>
        <input
          id={`${titleId}-password`}
          name="password"
          type="password"
          autoComplete="current-password"
          value={password}
          onChange={(event) => {
            setPassword(event.target.value);
          }}
        />
      </p>
      {formError === null ? null : <p role="alert">{formError}</p>}
      {saveState.status === 'saving' ? <p role="status">{t('field.saving')}</p> : null}
      {saveState.status === 'error' ? (
        <p role="alert">{formatError(saveState.error, translator)}</p>
      ) : null}
      <p className="form-actions">
        <button type="submit" disabled={saveState.status === 'saving'}>
          {t('auth.submit')}
        </button>
      </p>
      <p className="sign-in-hint">
        <a href="#/forgot">{t('auth.forgot')}</a>
      </p>
    </form>
  );
};

/** The API key pair, the programmatic path, behind its own disclosure. */
const KeySignIn = ({ onSignedIn }: SignInProps): ReactElement => {
  const { t } = useTranslator();
  const [identity, setIdentity] = useState('');
  const [credential, setCredential] = useState('');
  const [errors, setErrors] = useState<KeyErrors>({});
  const titleId = useId();
  const errorId = useId();

  const submit = (event: SyntheticEvent): void => {
    event.preventDefault();
    const parsed = parseApiKey(identity, credential);
    if (!parsed.ok) {
      setErrors(parsed.errors);
      return;
    }
    setErrors({});
    setIdentity('');
    setCredential('');
    signIn(parsed.key);
    onSignedIn?.();
  };

  const invalid = errors.identity !== undefined || errors.credential !== undefined;
  const message = [errors.identity, errors.credential]
    .filter((text) => text !== undefined)
    .join(' ');

  return (
    <form className="sign-in-form" aria-labelledby={titleId} onSubmit={submit}>
      <h3 className="sign-in-title" id={titleId}>
        {t('auth.heading')}
      </h3>
      <p>
        <label htmlFor={`${titleId}-identity`}>{t('auth.identity')}</label>
        <input
          id={`${titleId}-identity`}
          name="key_identity"
          value={identity}
          autoComplete="off"
          spellCheck={false}
          aria-invalid={errors.identity !== undefined}
          aria-describedby={errors.identity === undefined ? undefined : errorId}
          onChange={(event) => {
            setIdentity(event.target.value);
          }}
        />
      </p>
      <p>
        <label htmlFor={`${titleId}-credential`}>{t('auth.credential')}</label>
        <input
          id={`${titleId}-credential`}
          name="key_credential"
          type="password"
          value={credential}
          autoComplete="off"
          spellCheck={false}
          aria-invalid={errors.credential !== undefined}
          aria-describedby={errors.credential === undefined ? undefined : errorId}
          onChange={(event) => {
            setCredential(event.target.value);
          }}
        />
      </p>
      {invalid ? (
        <p role="alert" id={errorId}>
          {message}
        </p>
      ) : null}
      <p className="form-actions">
        <button type="submit">{t('auth.submit')}</button>
      </p>
      <p className="sign-in-hint">
        {t('auth.hint')} <code>romeka key add</code>.
      </p>
    </form>
  );
};

/** The panel a signed-out caller gets: the two forms, the key one folded away. */
export const SignInPanel = ({ onSignedIn }: SignInProps): ReactElement => {
  const { t } = useTranslator();
  return (
    <div className="sign-in-panel-body">
      <PasswordSignIn onSignedIn={onSignedIn} />
      <details className="sign-in-key">
        <summary>{t('auth.keyDisclosure')}</summary>
        <KeySignIn onSignedIn={onSignedIn} />
      </details>
    </div>
  );
};

/**
 * The header state. It names the signed-in user, or the key identity when the
 * session is the key pair alone, and offers the panel when neither is present.
 *
 * Signing out ends the server's session first, because the session row is the
 * server's to revoke, and then clears the key and the user on the client. A
 * refused logout renders its message and leaves the caller signed in.
 */
export const AuthBadge = (): ReactElement => {
  const translator = useTranslator();
  const { t } = translator;
  const key = useApiKey();
  const user = useSession();
  // A reader sent here by the public site's Log in link came to sign in.
  const [open, setOpen] = useState(() => signInReturnPath(window.location.search) !== null);
  const { state: saveState, save } = useSave();

  if (user !== null || key !== null) {
    const identity = user?.name ?? key?.identity ?? '';
    return (
      <span className="auth-badge">
        <span className="auth-identity">{t('auth.signedInAs', { identity })}</span>
        <button
          type="button"
          disabled={saveState.status === 'saving'}
          onClick={() => {
            save(async () => {
              await api.logout();
              endSession();
            });
          }}
        >
          {t('auth.signOut')}
        </button>
        {saveState.status === 'error' ? (
          <p role="alert">{formatError(saveState.error, translator)}</p>
        ) : null}
      </span>
    );
  }

  return (
    <span className="auth-badge">
      <span className="auth-identity">{t('auth.signedOut')}</span>
      <button
        type="button"
        aria-expanded={open}
        onClick={() => {
          setOpen(!open);
        }}
      >
        {t('auth.signIn')}
      </button>
      {open ? (
        <SignInPanel
          onSignedIn={() => {
            setOpen(false);
          }}
        />
      ) : null}
    </span>
  );
};
