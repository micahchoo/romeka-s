/**
 * Whether a schema field's control shows, from its `when` test on a sibling.
 * Hiding never clears a stored value: a field that comes back shows what it
 * held, and the server reads only what its layout uses.
 */
import type { JsonValue } from '../../../../api/types';
import type { BlockField } from '../../blockSchema';

/** A value the `set` test counts as present: not empty, not zero, not false. */
const isPresent = (value: JsonValue | undefined): boolean => {
  if (value === undefined || value === null || value === false || value === 0 || value === '') {
    return false;
  }
  return !Array.isArray(value) || value.length > 0;
};

/**
 * The sibling's stored value, or its schema default when it stores none, so a
 * new block reads the same as the form it starts from.
 */
const siblingValue = (
  key: string,
  record: Readonly<Record<string, JsonValue>>,
  siblings: readonly BlockField[],
): JsonValue | undefined => {
  const stored = record[key];
  if (stored !== undefined && stored !== null) {
    return stored;
  }
  const fallback = siblings.find((field) => field.key === key)?.default ?? null;
  if (fallback === null) {
    return undefined;
  }
  return typeof fallback === 'object' ? [...fallback] : fallback;
};

export const isShown = (
  field: BlockField,
  record: Readonly<Record<string, JsonValue>>,
  siblings: readonly BlockField[],
): boolean => {
  const when = field.when;
  if (when === null) {
    return true;
  }
  const value = siblingValue(when.field, record, siblings);
  switch (when.test) {
    case 'equals':
      return value === when.value;
    case 'set':
      return isPresent(value);
  }
};
