/**
 * The built-in vocabularies' comments in the active locale, beside their
 * labels. The comments namespace waits for its first read, so only a view
 * that shows a definition fetches them, even when its route's module graph
 * holds one that does (`termText.ts`).
 */
import { useMemo } from 'react';
import type { Locale } from './locale';
import { tableFor } from './namespace';
import { useLocale } from './store';
import { asStored, defineTermTable, translateWith, useTermTable, vocabularyLabels } from './termText';
import type { TermText } from './termText';

/** The built-in vocabulary comments. A locale without its file is a `tsc` error. */
export const vocabularyComments = defineTermTable(
  'vocabulary-comments',
  () => import('./locales/vocabulary/comments/en.json'),
  {
    es: () => import('./locales/vocabulary/comments/es.json'),
    hi: () => import('./locales/vocabulary/comments/hi.json'),
    mr: () => import('./locales/vocabulary/comments/mr.json'),
    ml: () => import('./locales/vocabulary/comments/ml.json'),
    ta: () => import('./locales/vocabulary/comments/ta.json'),
    kn: () => import('./locales/vocabulary/comments/kn.json'),
    te: () => import('./locales/vocabulary/comments/te.json'),
    ne: () => import('./locales/vocabulary/comments/ne.json'),
  },
  { onFirstRead: true },
);

/**
 * The built-in vocabularies' labels and comments in the active locale. The
 * caller suspends until both tables have arrived.
 */
export const useTermText = (): TermText => {
  const locale = useLocale();
  const labels = useTermTable(vocabularyLabels, locale);
  const comments = useTermTable(vocabularyComments, locale);
  return useMemo(
    () => ({ locale, label: translateWith(labels), comment: translateWith(comments) }),
    [locale, labels, comments],
  );
};

/** Fetch a locale's labels and comments; resolves once both have arrived. */
export const loadTermText = async (locale: Locale): Promise<void> => {
  await Promise.all([tableFor(vocabularyLabels, locale), tableFor(vocabularyComments, locale)]);
};

/**
 * The text of a locale outside a component: translated once its tables have
 * arrived, the stored text before that.
 */
export const termText = (locale: Locale): TermText => {
  const labels = tableFor(vocabularyLabels, locale);
  const comments = tableFor(vocabularyComments, locale);
  return {
    locale,
    label: labels instanceof Promise ? asStored : translateWith(labels),
    comment: comments instanceof Promise ? asStored : translateWith(comments),
  };
};
