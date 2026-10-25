/**
 * The value edits of a bulk change (`crates/omeka-api/src/bulk/values.rs`): add a value to a
 * field, replace one exact text with another, remove a field's values, or make them public or
 * private. A field takes at most one of the last three, and then any number of values to add;
 * the server applies them in that order whatever the order of the list.
 */
import { asCount, asFlag, list, member, nullable, oneOf, strict } from './import-read';
import { asIdNumber, asString, brandId } from './parse';
import type { ItemId, Parser, Path, PropertyId } from './types';

export type NewValue =
  | { readonly type: 'literal'; readonly text: string; readonly lang: string | null }
  | { readonly type: 'uri'; readonly uri: string; readonly label: string | null }
  | { readonly type: 'item'; readonly item: ItemId };

export type ValueOp =
  | { readonly op: 'add'; readonly property: PropertyId; readonly value: NewValue; readonly isPublic: boolean }
  | { readonly op: 'replace'; readonly property: PropertyId; readonly from: string; readonly to: string }
  | { readonly op: 'clear'; readonly property: PropertyId }
  | { readonly op: 'visibility'; readonly property: PropertyId; readonly isPublic: boolean };

/** The ops that change what a field holds, of which a field takes one. */
export const isRewrite = (op: ValueOp): boolean => op.op !== 'add';

const sameValue = (a: NewValue, b: NewValue): boolean => JSON.stringify(a) === JSON.stringify(b);

/**
 * The list with `op` in it. A second rewrite of one field takes the first one's place, and a
 * value the field already gets is not added twice, so the list is always one the server takes.
 */
export const withOp = (ops: readonly ValueOp[], op: ValueOp): readonly ValueOp[] => {
  if (op.op === 'add') {
    const twice = ops.some(
      (other) => other.op === 'add' && other.property === op.property && sameValue(other.value, op.value),
    );
    return twice ? ops : [...ops, op];
  }
  return [...ops.filter((other) => !(other.property === op.property && isRewrite(other))), op];
};

const asProperty = (value: unknown, path: Path): PropertyId => brandId<'PropertyId'>(asIdNumber(value, path));

const parseNewValue: Parser<NewValue> = (raw, path = []) => {
  const object = strict(raw, path, ['type'], ['text', 'lang', 'uri', 'label', 'item']);
  const type = member(object, 'type', path, oneOf(['literal', 'uri', 'item'] as const));
  const optional = (key: string): string | null =>
    key in object ? member(object, key, path, nullable(asString)) : null;
  switch (type) {
    case 'literal':
      return { type, text: member(object, 'text', path, asString), lang: optional('lang') };
    case 'uri':
      return { type, uri: member(object, 'uri', path, asString), label: optional('label') };
    case 'item':
      return { type, item: brandId<'ItemId'>(member(object, 'item', path, asIdNumber)) };
  }
};

const parseOp: Parser<ValueOp> = (raw, path = []) => {
  const object = strict(raw, path, ['op', 'property'], ['value', 'isPublic', 'from', 'to']);
  const op = member(object, 'op', path, oneOf(['add', 'replace', 'clear', 'visibility'] as const));
  const property = member(object, 'property', path, asProperty);
  switch (op) {
    case 'add':
      return {
        op,
        property,
        value: member(object, 'value', path, parseNewValue),
        isPublic: 'isPublic' in object ? member(object, 'isPublic', path, asFlag) : true,
      };
    case 'replace':
      return { op, property, from: member(object, 'from', path, asString), to: member(object, 'to', path, asString) };
    case 'clear':
      return { op, property };
    case 'visibility':
      return { op, property, isPublic: member(object, 'isPublic', path, asFlag) };
  }
};

/** The list as `BulkChange::to_json` stores it, which is also what the preview request sends. */
export const parseValueOps: Parser<readonly ValueOp[]> = list(parseOp);

/** One value as a preview line shows it. */
export interface ShownValue {
  /** The text, a web address's label, or the linked item's title. */
  readonly text: string | null;
  readonly lang: string | null;
  readonly uri: string | null;
  readonly resource: number | null;
  readonly isPublic: boolean;
}

/** A field whose values change on a sample item. */
export interface FieldChange {
  readonly property: PropertyId;
  readonly term: string;
  readonly label: string;
  readonly before: readonly ShownValue[];
  readonly after: readonly ShownValue[];
}

const parseShown: Parser<ShownValue> = (raw, path = []) => {
  const object = strict(raw, path, ['text', 'lang', 'uri', 'resource', 'isPublic']);
  return {
    text: member(object, 'text', path, nullable(asString)),
    lang: member(object, 'lang', path, nullable(asString)),
    uri: member(object, 'uri', path, nullable(asString)),
    resource: member(object, 'resource', path, nullable(asCount)),
    isPublic: member(object, 'isPublic', path, asFlag),
  };
};

export const parseFieldChange: Parser<FieldChange> = (raw, path = []) => {
  const object = strict(raw, path, ['property', 'term', 'label', 'before', 'after']);
  return {
    property: member(object, 'property', path, asProperty),
    term: member(object, 'term', path, asString),
    label: member(object, 'label', path, asString),
    before: member(object, 'before', path, list(parseShown)),
    after: member(object, 'after', path, list(parseShown)),
  };
};
