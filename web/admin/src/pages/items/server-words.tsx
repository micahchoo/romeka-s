/**
 * Messages the server wrote about a run (a failure's reason, a run's error), each a paragraph in
 * the reader's locale. The server writes English; the server catalogue translates it once its
 * chunk loads, and until then the English shows.
 */
import type { ReactElement } from 'react';
import { useAsync } from '../../app/useAsync';
import { useLocale } from '../../i18n';
import { serverTranslator } from '../../i18n/serverText';

export const ServerWords = ({ messages }: { readonly messages: readonly string[] }): ReactElement => {
  const locale = useLocale();
  const translated = useAsync(() => serverTranslator(locale), [locale]);
  const translate = translated.status === 'ready' ? translated.data : (message: string): string => message;
  return (
    <>
      {messages.map((message) => (
        <p key={message}>{translate(message)}</p>
      ))}
    </>
  );
};
