/**
 * Boundary parsing for the harness. The API documents and the harness state file
 * arrive as `unknown`, so every field is read through one of these helpers and a
 * wrong shape names the path that failed, the way `src/api/parse.ts` does.
 */

export const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

/**
 * `Array.isArray` narrows `unknown` to `any[]`, which spreads `any` through the
 * caller. This guard keeps the elements `unknown`.
 */
export const isArray = (value: unknown): value is readonly unknown[] => Array.isArray(value);

export const record = (value: unknown, path: string): Record<string, unknown> => {
  if (!isRecord(value)) {
    throw new Error(`${path}: expected an object`);
  }
  return value;
};

export const stringField = (
  doc: Record<string, unknown>,
  key: string,
  path: string,
): string => {
  const value = doc[key];
  if (typeof value !== 'string') {
    throw new Error(`${path}.${key}: expected a string`);
  }
  return value;
};

export const numberField = (
  doc: Record<string, unknown>,
  key: string,
  path: string,
): number => {
  const value = doc[key];
  if (typeof value !== 'number') {
    throw new Error(`${path}.${key}: expected a number`);
  }
  return value;
};

export const booleanField = (
  doc: Record<string, unknown>,
  key: string,
  path: string,
): boolean => {
  const value = doc[key];
  if (typeof value !== 'boolean') {
    throw new Error(`${path}.${key}: expected a boolean`);
  }
  return value;
};

export const parseJson = (text: string): unknown => {
  const value: unknown = JSON.parse(text);
  return value;
};
