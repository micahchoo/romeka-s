import type { JsonValue } from '../api/types';

/** Pretty JSON for an editor textarea. `[]` and `{}` stay on one line. */
export const formatJson = (value: JsonValue): string => JSON.stringify(value, null, 2);
