/**
 * A reason the server wrote (a failure's `detail`, a kept resource's reason)
 * in the reader's language. The server writes English, and the English is the
 * message's id, so the server catalogue translates it as it does error text
 * (`i18n/serverText.ts`). Until that catalogue arrives the English shows.
 */
import { useEffect, useState } from 'react';
import { useLocale } from '../../../i18n';
import { serverTranslator } from '../../../i18n/serverText';
import type { ServerTranslate } from '../../../i18n/serverText';

const english: ServerTranslate = (message) => message;

export const useServerWords = (): ServerTranslate => {
  const locale = useLocale();
  const [translate, setTranslate] = useState<{ readonly locale: string; readonly translate: ServerTranslate } | null>(null);
  useEffect(() => {
    let current = true;
    void serverTranslator(locale).then((next) => {
      if (current) {
        setTranslate({ locale, translate: next });
      }
    });
    return () => {
      current = false;
    };
  }, [locale]);
  return translate?.locale === locale ? translate.translate : english;
};
