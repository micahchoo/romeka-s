/**
 * The marks in words, for the sentence "Files are one item when their names
 * differ only in the part you marked: …".
 */
import type { Mark } from '../example';
import type { SimpleTranslator } from './messages';

const quoted = (words: readonly string[]): string => words.map((word) => `“${word}”`).join(', ');

export const markWords = (mark: Mark, t: SimpleTranslator['t']): string => {
  switch (mark.kind) {
    case 'number':
      return t('guided.mark.number', { text: `“${mark.text}”` });
    case 'letters':
      return t('guided.mark.letters', { text: `“${mark.text}”` });
    case 'words':
      return quoted(mark.words);
  }
};

export const marksWords = (marks: readonly Mark[], t: SimpleTranslator['t']): string =>
  marks.map((mark) => markWords(mark, t)).join(t('guided.mark.and'));
