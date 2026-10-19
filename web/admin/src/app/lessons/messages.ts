/**
 * The lessons' messages, read through the `lessons` namespace
 * (`src/i18n/namespaces/lessons/`). A lesson never holds up its page, so this
 * lookup does not suspend: it is `null`, and the lesson renders nothing, until
 * the table arrives, and the arrival renders the caller again.
 */
import { useEffect, useMemo, useState } from 'react';
import { tableFor, translatorOver, useLocale } from '../../i18n';
import type { KeyOf, MessageParams } from '../../i18n';
import { lessons } from '../../i18n/namespaces/lessons';

export { en } from '../../i18n/namespaces/lessons';
export type { LessonsMessages } from '../../i18n/namespaces/lessons';

/** A lessons message id. */
export type LessonsMessageKey = KeyOf<typeof lessons>;

/** A lookup into the lessons table. */
export type LessonsText = (key: LessonsMessageKey, params?: MessageParams) => string;

/** The active locale's lookup, or `null` until its table arrives. */
export const useLessonsText = (): LessonsText | null => {
  const locale = useLocale();
  const table = tableFor(lessons, locale);
  const [, setArrivals] = useState(0);
  useEffect(() => {
    if (!(table instanceof Promise)) {
      return undefined;
    }
    let live = true;
    table.then(
      () => {
        if (live) {
          setArrivals((count) => count + 1);
        }
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [table]);
  return useMemo(
    () => (table instanceof Promise ? null : translatorOver(locale, [table]).t),
    [locale, table],
  );
};
