/**
 * A group's facts travel as a filter string (`JSON.stringify(facts)`), in the
 * filter row and in an address. This reads one back, refusing any string that
 * is not a group's facts, so a hand-edited address narrows nothing rather than
 * sending the server a shape it refuses.
 */
import { BUCKETS, MEDIA_KINDS } from '../../../api/import-sections';
import type { Bucket, MediaKind, ShapeFacts } from '../../../api/import-sections';
import { isRecord } from '../../../api/parse';

const strings = (value: unknown): readonly string[] | null =>
  Array.isArray(value) && value.every((item) => typeof item === 'string') ? value.map(String) : null;

const bucket = (value: unknown): Bucket | null => BUCKETS.find((one) => one === value) ?? null;

const media = (value: unknown): readonly MediaKind[] | null => {
  const words = strings(value);
  if (words === null) {
    return null;
  }
  const kinds = words.flatMap((word) => MEDIA_KINDS.filter((kind) => kind === word));
  return kinds.length === words.length ? kinds : null;
};

export const shapeKey = (facts: ShapeFacts): string => JSON.stringify(facts);

export const shapeFromKey = (key: string): ShapeFacts | null => {
  let raw: unknown;
  try {
    raw = JSON.parse(key);
  } catch {
    return null;
  }
  if (!isRecord(raw)) {
    return null;
  }
  const files = bucket(raw['files']);
  const folders = strings(raw['folders']);
  const kinds = media(raw['media']);
  const parts = raw['parts'] === null ? null : strings(raw['parts']);
  const pages = raw['pages'] === null ? null : strings(raw['pages']);
  const { pattern, row } = raw;
  if (
    files === null ||
    folders === null ||
    kinds === null ||
    typeof pattern !== 'boolean' ||
    typeof row !== 'boolean' ||
    (raw['parts'] !== null && parts === null) ||
    (raw['pages'] !== null && pages === null)
  ) {
    return null;
  }
  return { files, folders, media: kinds, pattern, row, parts, pages };
};
