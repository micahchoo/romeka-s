/**
 * A place: the `"lat, lng"` literal a site's location field holds. The public
 * site reads the same form (`crates/omeka-site/src/map.rs`, `Place`), and both
 * sides store at most six decimals, about 10 cm.
 */

export interface Place {
  readonly lat: number;
  readonly lng: number;
}

/** OpenStreetMap's tiles and the credit they require, the site setting defaults. */
export const DEFAULT_TILES = 'https://tile.openstreetmap.org/{z}/{x}/{y}.png';
export const DEFAULT_ATTRIBUTION = '© OpenStreetMap contributors';

const DECIMAL = /^[+-]?(\d+(\.\d*)?|\.\d+)$/;

const degrees = (text: string): number | null => {
  const trimmed = text.trim();
  return DECIMAL.test(trimmed) ? Number(trimmed) : null;
};

/** A place from its parts, `null` outside -90 to 90 and -180 to 180. */
export const placeOf = (lat: number, lng: number): Place | null =>
  Math.abs(lat) <= 90 && Math.abs(lng) <= 180 ? { lat, lng } : null;

/** The place a value's text holds: latitude, a comma, longitude. */
export const parsePlace = (text: string): Place | null => {
  const [lat, lng, ...rest] = text.split(',');
  if (lat === undefined || lng === undefined || rest.length > 0) {
    return null;
  }
  const latDegrees = degrees(lat);
  const lngDegrees = degrees(lng);
  return latDegrees === null || lngDegrees === null ? null : placeOf(latDegrees, lngDegrees);
};

const coordinate = (value: number): string => {
  const trimmed = value.toFixed(6).replace(/0+$/, '').replace(/\.$/, '');
  return trimmed === '-0' ? '0' : trimmed;
};

/** The stored form of a place. */
export const formatPlace = (place: Place): string =>
  `${coordinate(place.lat)}, ${coordinate(place.lng)}`;

/** A tile template that can draw a map; anything else falls back to OpenStreetMap's. */
export const tileTemplate = (stored: unknown): string =>
  typeof stored === 'string' &&
  /^https?:\/\//.test(stored) &&
  ['{z}', '{x}', '{y}'].every((part) => stored.includes(part))
    ? stored
    : DEFAULT_TILES;
