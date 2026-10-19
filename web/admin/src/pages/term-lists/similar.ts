/**
 * Which spellings look like one value: the rule behind the merge prompt.
 *
 * Two spellings are alike when their likeness keys are equal. The key ignores
 * case, Latin accents, punctuation and a few place-name affixes ("Co.",
 * "County", "City", "the"), so "Cork", "Co. Cork" and "Cork City" share one.
 * Only the Latin combining accents (U+0300 to U+036F) are removed: the vowel
 * signs of Indic scripts are marks too, and dropping them would change the
 * word. The rule only suggests; the user picks the kept spelling or says the
 * spellings are different, and that answer is remembered on the server.
 */
import type { ValueCount } from '../../api/tags';

const AFFIXES: ReadonlySet<string> = new Set(['co', 'county', 'city', 'the']);

export const likenessKey = (text: string): string => {
  const words = text
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .normalize('NFC')
    .toLowerCase()
    .split(/[^\p{L}\p{M}\p{N}]+/u)
    .filter((word) => word !== '');
  const core = words.filter((word) => !AFFIXES.has(word));
  return (core.length > 0 ? core : words).join(' ');
};

/** A group already called different covers every spelling in this one. */
const calledDifferent = (
  group: readonly ValueCount[],
  distinct: readonly (readonly string[])[],
): boolean =>
  distinct.some((known) => group.every((count) => known.includes(count.value)));

/**
 * The groups of two or more alike spellings, each most used first and, on a
 * tie, shortest first (the bare "Cork" before "Co. Cork"), the groups touching
 * the most resources first. A group the user called different is left out
 * until a new spelling joins it.
 */
export const similarGroups = (
  counts: readonly ValueCount[],
  distinct: readonly (readonly string[])[],
): readonly (readonly ValueCount[])[] => {
  const byKey = new Map<string, ValueCount[]>();
  for (const count of counts) {
    const key = likenessKey(count.value);
    byKey.set(key, [...(byKey.get(key) ?? []), count]);
  }
  const total = (group: readonly ValueCount[]): number =>
    group.reduce((sum, count) => sum + count.resources, 0);
  return [...byKey.values()]
    .filter((group) => group.length > 1 && !calledDifferent(group, distinct))
    .map((group) =>
      [...group].sort((a, b) => b.resources - a.resources || a.value.length - b.value.length),
    )
    .sort((a, b) => total(b) - total(a));
};
