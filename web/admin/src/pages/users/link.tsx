/**
 * The one-time password link an administrator hands to a user.
 *
 * `POST /api/users/<id>/password-link` answers the key, the URL that carries it
 * and whether the invitation mail went out, so the panel names all three. With
 * no SMTP service configured, `sent` is false and the URL is the only way the
 * user can set a password.
 */
import { useId, useState } from 'react';
import type { ReactElement } from 'react';
import type { PasswordLink } from '../../api/types';
import { useTranslator } from '../../i18n';

export interface PasswordLinkPanelProps {
  readonly name: string;
  readonly email: string;
  readonly link: PasswordLink;
}

type CopyState = 'idle' | 'copied' | 'failed';

export const PasswordLinkPanel = ({
  name,
  email,
  link,
}: PasswordLinkPanelProps): ReactElement => {
  const { t } = useTranslator();
  const [copied, setCopied] = useState<CopyState>('idle');
  const titleId = useId();

  const copy = (): void => {
    navigator.clipboard.writeText(link.link).then(
      () => {
        setCopied('copied');
      },
      () => {
        setCopied('failed');
      },
    );
  };

  return (
    <section className="password-link" aria-labelledby={titleId}>
      <h2 id={titleId}>{t('users.linkLegend')}</h2>
      <p role="status">
        {link.sent ? t('users.linkMailed', { email }) : t('users.linkNotMailed')}
      </p>
      <p>
        <label htmlFor={`${titleId}-url`}>{t('users.linkLabel', { name })}</label>
        <input id={`${titleId}-url`} type="text" readOnly value={link.link} />
        <button type="button" onClick={copy}>
          {t('users.copy')}
        </button>
      </p>
      {copied === 'copied' ? <p role="status">{t('users.copied')}</p> : null}
      {copied === 'failed' ? <p role="alert">{t('users.copyFailed')}</p> : null}
    </section>
  );
};
