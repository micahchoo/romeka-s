/**
 * The field values wire (`crates/omeka-api/src/field_values.rs`): the census, the move answer,
 * and the undo a move answer names.
 */
import { describe, expect, it } from 'vitest';
import { brandId } from '../src/api/parse';
import { parseFieldValues, parseMoved, undoOf } from '../src/api/field-values';

const SUBJECT = brandId<'PropertyId'>(3);
const PLACE = brandId<'PropertyId'>(40);

describe('the field values wire', () => {
  it('reads a census, most used first as sent', () => {
    expect(
      parseFieldValues({
        property: 3,
        values: [
          { value: 'Bidar', items: 72, media: 3, item_sets: 0, editable: 75 },
          { value: 'Coin', items: 0, media: 212, item_sets: 0, editable: 0 },
        ],
      }),
    ).toEqual({
      property: 3,
      values: [
        { value: 'Bidar', items: 72, media: 3, itemSets: 0, editable: 75 },
        { value: 'Coin', items: 0, media: 212, itemSets: 0, editable: 0 },
      ],
    });
  });

  it('reads a move answer with the item types whose form lacks the field', () => {
    expect(
      parseMoved({
        changed: 3,
        skipped: 1,
        moved: [10, 11],
        merged: [12],
        copied: [],
        not_in_form: [{ template: 7, label: 'Photo Narrative', resources: 2 }],
      }),
    ).toEqual({
      changed: 3,
      skipped: 1,
      moved: [10, 11],
      merged: [12],
      copied: [],
      notInForm: [{ template: 7, label: 'Photo Narrative', resources: 2 }],
    });
  });

  it('undoes a move by moving back on the moved records and copying back on the merged ones', () => {
    const move = { value: 'Bidar', from: SUBJECT, to: PLACE };
    const answer = parseMoved({
      changed: 3,
      skipped: 0,
      moved: [10, 11],
      merged: [12],
      copied: [],
      not_in_form: [],
    });
    expect(undoOf(move, answer)).toEqual({
      value: 'Bidar',
      from: PLACE,
      to: SUBJECT,
      only: [10, 11],
      copy: [12],
    });
  });
});
