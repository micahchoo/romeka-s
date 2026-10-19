/**
 * The forgot-password surface.
 *
 * `POST /api/password/forgot` answers 204 whether or not the address matches an
 * account, so the route cannot be used to enumerate users. The page says the
 * same thing for every address it is given, and that sameness is the point of
 * the copy rather than an accident of it.
 */
import { useId, useState } from 'react';
import type { ReactElement, SyntheticEvent } from 'react';
import { api } from '../api/client';
import { useTranslator } from '../i18n';
import { formatError } from './format';
import { useSave } from './save';
import { account } from '../i18n/namespaces/account';

export const ForgotPasswordPage = (): ReactElement => {
  const translator = useTranslator(account);
  const { t } = translator;
  const [email, setEmail] = useState('');
  const [formError, setFormError] = useState<string | null>(null);
  const { state: saveState, save } = useSave();
  const titleId = useId();

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (email.trim() === '') {
      setFormError(t('forgot.errorEmpty'));
      return;
    }
    setFormError(null);
    save(async () => {
      await api.forgotPassword(email);
      setEmail('');
    });
  };

  return (
    <section className="auth-page" aria-labelledby={titleId}>
      <h1 id={titleId}>{t('forgot.heading')}</h1>
      {saveState.status === 'saved' ? (
        <p role="status">{t('forgot.sent')}</p>
      ) : (
        <form aria-labelledby={titleId} onSubmit={submit}>
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
          {formError === null ? null : <p role="alert">{formError}</p>}
          {saveState.status === 'saving' ? <p role="status">{t('field.saving')}</p> : null}
          {saveState.status === 'error' ? (
            <p role="alert">{formatError(saveState.error, translator)}</p>
          ) : null}
          <p className="form-actions">
            <button type="submit" disabled={saveState.status === 'saving'}>
              {t('forgot.submit')}
            </button>
          </p>
        </form>
      )}
      <p className="sign-in-hint">{t('forgot.hint')}</p>
    </section>
  );
};
