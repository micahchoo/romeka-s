/**
 * The panel's reader words against the server's. The probe answers one of the
 * readers `omeka_api::media_add` can name, and the panel reads that answer
 * (`api/staged-uploads.ts`); a reader the server adds with no word here would
 * reach a row as an empty string, so the two lists must agree.
 *
 * The design asked this test to compare Import's `source-words.ts`, but that
 * file names plan sources (columns, folders, tags) and holds no ingester
 * words, so the server's list is the other side instead.
 */
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { PROBE_INGESTERS } from '../src/api/staged-uploads';
import type { Ingester } from '../src/api/staged-uploads';
import { en } from '../src/i18n/namespaces/media-add';
import { INGESTER_WORD, READING_WORD, SUBMIT_LABEL } from '../src/pages/items/add-media/ingesters';

const HERE = dirname(fileURLToPath(import.meta.url));
const SERVER = resolve(HERE, '../../../crates/omeka-api/src/media_add.rs');

const serverReaders = (): readonly string[] => {
  const source = readFileSync(SERVER, 'utf8');
  const names = new Set<string>();
  for (const list of source.matchAll(/vec!\[([^\]]*)\]/gu)) {
    for (const name of (list[1] ?? '').matchAll(/"([a-z_]+)"/gu)) {
      if (name[1] !== undefined) {
        names.add(name[1]);
      }
    }
  }
  return [...names].sort();
};

const words = (table: Readonly<Record<Ingester, keyof typeof en>>): readonly string[] =>
  Object.values(table).map((key) => en[key]);

describe('the reader words', () => {
  it('covers every reader the server can answer, plus pasted HTML', () => {
    expect(serverReaders()).toEqual([...PROBE_INGESTERS].sort());
    const offered: Ingester[] = [...PROBE_INGESTERS, 'html'];
    for (const ingester of offered) {
      expect(INGESTER_WORD[ingester], ingester).toBeDefined();
      expect(SUBMIT_LABEL[ingester], ingester).toBeDefined();
      expect(READING_WORD[ingester], ingester).toBeDefined();
    }
  });

  it('names the reader and the press separately, so the button states the effect', () => {
    expect(words(INGESTER_WORD)).toEqual([
      'Download the file',
      'YouTube video',
      'oEmbed page',
      'IIIF image',
      'IIIF manifest',
      'HTML',
    ]);
    expect(words(SUBMIT_LABEL)).toEqual([
      'Download and add file',
      'Add YouTube video',
      'Add oEmbed page',
      'Add IIIF image',
      'Add IIIF manifest',
      'Add HTML',
    ]);
  });

  it('has no two readers named alike', () => {
    const labels = words(INGESTER_WORD);
    expect(new Set(labels).size).toBe(labels.length);
  });
});
