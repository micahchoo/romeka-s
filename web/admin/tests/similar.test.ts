/**
 * The near-duplicate rule behind the merge prompt: which spellings look like
 * one value, and which groups the user already called different.
 */
import { describe, expect, it } from 'vitest';
import { likenessKey, similarGroups } from '../src/pages/term-lists/similar';

const count = (value: string, resources: number) => ({ value, resources, editable: resources });

describe('likeness key', () => {
  it('ignores_case_accents_punctuation_and_place_affixes', () => {
    const cork = likenessKey('Cork');
    expect(likenessKey('Co. Cork')).toBe(cork);
    expect(likenessKey('Cork City')).toBe(cork);
    expect(likenessKey('CORK,')).toBe(cork);
    expect(likenessKey('County Cork')).toBe(cork);
    expect(likenessKey('Dún Laoghaire')).toBe(likenessKey('dun laoghaire'));
    expect(likenessKey('the farm')).toBe(likenessKey('Farm'));
  });

  it('keeps_different_words_apart', () => {
    expect(likenessKey('Cork')).not.toBe(likenessKey('Corke'));
    expect(likenessKey('New York')).not.toBe(likenessKey('York'));
    expect(likenessKey('wartime')).not.toBe(likenessKey('war time'));
  });

  it('keeps_a_value_made_only_of_affixes', () => {
    expect(likenessKey('The City')).toBe('the city');
  });

  it('keeps_the_vowel_signs_of_indic_scripts', () => {
    expect(likenessKey('कि')).not.toBe(likenessKey('क'));
    expect(likenessKey('காடு')).toBe('காடு');
  });
});

describe('similar groups', () => {
  it('groups_alike_spellings_most_used_first', () => {
    const groups = similarGroups(
      [count('Boston', 9), count('Co. Cork', 3), count('Cork', 6), count('Cork City', 1)],
      [],
    );
    expect(groups).toEqual([[count('Cork', 6), count('Co. Cork', 3), count('Cork City', 1)]]);
  });

  it('offers_the_shortest_spelling_first_when_uses_tie', () => {
    expect(similarGroups([count('Co. Cork', 1), count('Cork', 1)], [])).toEqual([
      [count('Cork', 1), count('Co. Cork', 1)],
    ]);
  });

  it('drops_a_group_the_user_called_different', () => {
    const counts = [count('Cork', 6), count('Cork City', 1)];
    expect(similarGroups(counts, [['Cork', 'Cork City']])).toEqual([]);
    expect(similarGroups(counts, [['Cork', 'Cork City', 'Co. Cork']])).toEqual([]);
  });

  it('asks_again_when_a_new_spelling_joins_a_group_called_different', () => {
    const counts = [count('Cork', 6), count('Cork City', 1), count('Co. Cork', 2)];
    expect(similarGroups(counts, [['Cork', 'Cork City']])).toEqual([
      [count('Cork', 6), count('Co. Cork', 2), count('Cork City', 1)],
    ]);
  });
});
