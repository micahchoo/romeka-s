/**
 * The set questions of the Structure clicks agree with the server's
 * (`plan/edits.rs`, `Question`): whether a set is made (`addSet`,
 * `dropSet`) is one question, and what it is (`title`, `existing`) is
 * another. So naming a set never takes back the click that made it.
 */
import { describe, expect, it } from 'vitest';
import type { Edit, SetFrom } from '../src/api/import-plan';
import { QUESTIONS, questionOf, withAnswer } from '../src/pages/import/simple/structure-edits';

const PLACE: SetFrom = { from: 'namePlace', scope: { source: 's1', folder: 'Documents (D)' }, place: 4 };
const FOLDER: SetFrom = { from: 'folder', scope: { source: 's1', folder: 'Documents (D)' } };
const ADD: Edit = { edit: 'addSet', from: PLACE };
const TITLE: Edit = { edit: 'title', from: PLACE, title: 'Dated', description: 'By the date in the name' };

describe('the set questions', () => {
  it('a title given to a set that a click made keeps that click', () => {
    expect(withAnswer([ADD], questionOf(TITLE), TITLE)).toEqual([ADD, TITLE]);
  });

  it('a set in Omeka answers the title question: the later of the two stays', () => {
    const existing: Edit = { edit: 'existing', from: PLACE, id: 42 };
    expect(questionOf(existing)).toBe(QUESTIONS.title(PLACE));
    expect(withAnswer([ADD, TITLE], questionOf(existing), existing)).toEqual([ADD, existing]);
  });

  it('taking the set away takes back what it was named, and nothing of another set', () => {
    const other: Edit = { edit: 'title', from: FOLDER, title: 'Papers' };
    const drop: Edit = { edit: 'dropSet', from: PLACE };
    expect(withAnswer([ADD, TITLE, other], QUESTIONS.set(PLACE), drop)).toEqual([other, drop]);
  });
});
