/**
 * The error surface every page load shares.
 *
 * A 403 means the API refused the caller, and the way out is a key, so a
 * forbidden page renders the sign-in form under its message. Any other error
 * keeps the message alone.
 */
import type { ReactElement } from 'react';
import { ApiError } from '../api/client';
import { useTranslator } from '../i18n';
import { SignInPanel } from './auth';
import { formatError } from './format';

const isForbidden = (error: Error): boolean =>
  error instanceof ApiError && error.status === 403;

export const ErrorNotice = ({ error }: { readonly error: Error }): ReactElement => {
  const translator = useTranslator();
  return (
    <>
      <p role="alert">{formatError(error, translator)}</p>
      {isForbidden(error) ? (
        <section className="sign-in-panel" aria-label={translator.t('auth.panelLabel')}>
          <SignInPanel />
        </section>
      ) : null}
    </>
  );
};
