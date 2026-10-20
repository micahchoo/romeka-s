/**
 * The colours a site's theme roles resolve to, for the swatches of a colour
 * field. It reads the theme resolver, so the colour field imports this module
 * on demand and the fields chunk does not carry the theme registry.
 */
import type { JsonValue } from '../../../../api/types';
import { draftFromStored, resolve } from '../../themeEditor/draft';
import { roleColour } from '../../themeEditor/palette';
import type { Role } from '../../themeEditor/palette';

const ROLES: readonly Role[] = [
  'brand',
  'on-brand',
  'accent',
  'background',
  'surface',
  'text',
  'muted',
  'link',
];

const isStored = (value: JsonValue | null): value is Readonly<Record<string, JsonValue>> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

/** Each role's colour in the shade the site shows first, keyed by the stored role name. */
export const sitePalette = (themeSettings: JsonValue | null): ReadonlyMap<string, string> => {
  const resolved = resolve(draftFromStored(isStored(themeSettings) ? themeSettings : {}));
  const shown = resolved.scheme === 'dark' ? resolved.dark : resolved.light;
  return new Map(ROLES.map((role) => [role, roleColour(shown, role)]));
};
