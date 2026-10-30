/**
 * One field's texts across the archive, and moving one text to another field
 * (`/api/romeka/field-values`, `crates/omeka-api/src/field_values.rs`). Only the values board
 * reads these, so this module stays out of the initial payload.
 */
import type { ApiClient } from './client';
import { asNumber, asRecord, asString, field, parseListOf } from './parse';
import type { Path, Parser, PropertyId } from './types';

/** One text of a field and the records the caller can see that hold it. */
export interface FieldValue {
  readonly value: string;
  readonly items: number;
  readonly media: number;
  readonly itemSets: number;
  /** Of all of those, the ones the caller may change. */
  readonly editable: number;
}

export interface FieldValues {
  readonly property: number;
  /** Most used first. */
  readonly values: readonly FieldValue[];
}

/** An item type whose form does not show the target field, and how many changed records use it. */
export interface NotInForm {
  readonly template: number;
  readonly label: string;
  readonly resources: number;
}

/** What a move did, or would do. */
export interface Moved {
  readonly changed: number;
  /** Records holding the text that the caller may not change; they keep it where it is. */
  readonly skipped: number;
  readonly moved: readonly number[];
  /** Records that already held the text on the target; they only lost it from the source. */
  readonly merged: readonly number[];
  readonly copied: readonly number[];
  readonly notInForm: readonly NotInForm[];
}

export interface Move {
  readonly value: string;
  readonly from: PropertyId;
  readonly to: PropertyId;
  /** Move only on these records. */
  readonly only?: readonly number[];
  /** On these records, copy the text to `to` and keep it on `from`. */
  readonly copy?: readonly number[];
}

const count = (object: Record<string, unknown>, name: string, path: Path): number =>
  asNumber(field(object, name, path), [...path, name]);

const ids = (object: Record<string, unknown>, name: string, path: Path): readonly number[] =>
  parseListOf(asNumber)(field(object, name, path), [...path, name]);

const parseFieldValue: Parser<FieldValue> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    value: asString(field(object, 'value', path), [...path, 'value']),
    items: count(object, 'items', path),
    media: count(object, 'media', path),
    itemSets: count(object, 'item_sets', path),
    editable: count(object, 'editable', path),
  };
};

export const parseFieldValues: Parser<FieldValues> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    property: count(object, 'property', path),
    values: parseListOf(parseFieldValue)(field(object, 'values', path), [...path, 'values']),
  };
};

const parseNotInForm: Parser<NotInForm> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    template: count(object, 'template', path),
    label: asString(field(object, 'label', path), [...path, 'label']),
    resources: count(object, 'resources', path),
  };
};

export const parseMoved: Parser<Moved> = (raw, path = []) => {
  const object = asRecord(raw, path);
  return {
    changed: count(object, 'changed', path),
    skipped: count(object, 'skipped', path),
    moved: ids(object, 'moved', path),
    merged: ids(object, 'merged', path),
    copied: ids(object, 'copied', path),
    notInForm: parseListOf(parseNotInForm)(field(object, 'not_in_form', path), [...path, 'not_in_form']),
  };
};

/** The move that puts every field back as `move` found it, from the answer `move` gave. */
export const undoOf = (move: Move, answer: Moved): Move => ({
  value: move.value,
  from: move.to,
  to: move.from,
  only: answer.moved,
  copy: answer.merged,
});

/** Every record's total of a value, whatever its kind. */
export const recordsOf = (value: FieldValue): number => value.items + value.media + value.itemSets;

export const getFieldValues = async (api: ApiClient, property: PropertyId): Promise<FieldValues> =>
  parseFieldValues(await api.requestJson(`/api/romeka/field-values/${String(property)}`));

export const moveFieldValue = async (api: ApiClient, move: Move, dryRun: boolean): Promise<Moved> =>
  parseMoved(
    await api.writeJson('POST', '/api/romeka/field-values/move', {
      value: move.value,
      from: move.from,
      to: move.to,
      dry_run: dryRun,
      ...(move.only === undefined ? {} : { only: [...move.only] }),
      ...(move.copy === undefined ? {} : { copy: [...move.copy] }),
    }),
  );
