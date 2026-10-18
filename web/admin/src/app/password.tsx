/**
 * The set-password surface: the page a one-time key reaches.
 *
 * The key arrives in the URL and the page never stores it, so it lasts one
 * submit. Upstream's `LoginController::createPasswordAction` hides it the same
 * way. The client checks only what the server cannot answer cheaply: an empty
 * password and a mistyped confirmation. The password rules and the key's
 * validity are the server's, and its 404 or 422 renders as it came.
 */
import { useId, useState } from 'react';
import type { ReactElement, SyntheticEvent } from 'react';
import { api } from '../api/client';
import { useTranslator } from '../i18n';
import { formatError } from './format';
import { useSave } from './save';

export interface PasswordPageProps {
  /** The one-time key the URL carried, or `null` when the link has none. */
  readonly passwordKey: string | null;
}

export const PasswordPage = ({ passwordKey }: PasswordPageProps): ReactElement => {
  const translator = useTranslator();
  const { t } = translator;
  const [password, setPassword] = useState('');
  const [confirmation, setConfirmation] = useState('');
  const [formError, setFormError] = useState<string | null>(null);
  const { state: saveState, save } = useSave();
  const titleId = useId();

  if (passwordKey === null) {
    return (
      <section className="auth-page" aria-labelledby={titleId}>
        <h1 id={titleId}>{t('password.heading')}</h1>
        <p role="alert">{t('password.missingKey')}</p>
        <p>
          <a href="#/forgot">{t('forgot.heading')}</a>
        </p>
      </section>
    );
  }

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (password === '') {
      setFormError(t('password.errorEmpty'));
      return;
    }
    if (password !== confirmation) {
      setFormError(t('password.errorMismatch'));
      return;
    }
    setFormError(null);
    save(async () => {
      await api.setPassword(passwordKey, password);
      setPassword('');
      setConfirmation('');
    });
  };

  return (
    <section className="auth-page" aria-labelledby={titleId}>
      <h1 id={titleId}>{t('password.heading')}</h1>
      {saveState.status === 'saved' ? (
        <p role="status">{t('password.done')}</p>
      ) : (
        <form aria-labelledby={titleId} onSubmit={submit}>
          <p>
            <label htmlFor={`${titleId}-new`}>{t('password.new')}</label>
            <input
              id={`${titleId}-new`}
              name="password"
              type="password"
              autoComplete="new-password"
              value={password}
              onChange={(event) => {
                setPassword(event.target.value);
              }}
            />
          </p>
          <p>
            <label htmlFor={`${titleId}-confirm`}>{t('password.confirm')}</label>
            <input
              id={`${titleId}-confirm`}
              name="password_confirmation"
              type="password"
              autoComplete="new-password"
              value={confirmation}
              onChange={(event) => {
                setConfirmation(event.target.value);
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
              {t('password.submit')}
            </button>
          </p>
        </form>
      )}
    </section>
  );
};
