/**
 * The example compiler (`pages/import/example.ts`) on real file names from
 * the archive-import prototype: the pattern it writes, the files that pattern
 * matches, and the readable shape.
 */
import { describe, expect, it } from 'vitest';
import type { ImportExample } from '../src/api/imports';
import {
  compileExample,
  defaultExample,
  labelsFor,
  optionalCandidates,
  splitExample,
  withLabel,
  withOptional,
} from '../src/pages/import/example';

const FRONT = 'Artifacts/BHC_006_AF_001_front.jpg';

/** The prototype's marks: top folder the set, `BHC_006_AF_001` the identifier, `front` the part. */
const marked: ImportExample = {
  path: FRONT,
  labels: { d0: 'set', n0: 'identifier', n2: 'identifier', n4: 'identifier', n6: 'identifier', n8: 'part' },
  optional: [],
};

const groupsOf = (example: ImportExample, path: string): Record<string, string> | null => {
  const match = new RegExp(compileExample(example).pattern).exec(path);
  return match === null ? null : { ...match.groups };
};

describe('splitExample', () => {
  it('cuts folders, name parts with their delimiters, and the extension', () => {
    const parts = splitExample('Photographs/1950s/BHC_006_PH_101.tif');
    expect(parts.folders).toEqual(['Photographs', '1950s']);
    expect(parts.ext).toBe('tif');
    expect(parts.name.map((part) => part.text)).toEqual(['BHC', '_', '006', '_', 'PH', '_', '101']);
    expect(parts.name.filter((part) => part.delimiter).map((part) => part.key)).toEqual(['n1', 'n3', 'n5']);
  });
});

describe('compileExample', () => {
  it('writes one identifier group across delimiters, with the part required', () => {
    const { pattern, shape } = compileExample(marked);
    expect(pattern).toBe(
      '^(?<set>[^/]+)/(?:[^/]+/)*(?<identifier>[A-Z]+_[0-9]{3}_[A-Z]+_[0-9]{3})_(?<part>[a-z]+)\\.(?<ext>[A-Za-z0-9]+)$',
    );
    expect(shape).toBe('{set}/…/{identifier}_{part}.{ext}');
    expect(groupsOf(marked, 'Artifacts/BHC_006_AF_002_back.jpg')).toEqual({
      set: 'Artifacts',
      identifier: 'BHC_006_AF_002',
      part: 'back',
      ext: 'jpg',
    });
    expect(groupsOf(marked, 'Photographs/1950s/BHC_006_PH_101.tif')).toBeNull();
  });

  it('with the part optional, matches BHC_006_PH_101.tif two folders deep', () => {
    const optional = withOptional(marked, 'part', true);
    expect(compileExample(optional).shape).toBe('{set}/…/{identifier}[_{part}].{ext}');
    expect(groupsOf(optional, 'Photographs/1950s/BHC_006_PH_101.tif')).toEqual({
      set: 'Photographs',
      identifier: 'BHC_006_PH_101',
      part: undefined,
      ext: 'tif',
    });
    expect(groupsOf(optional, 'Photographs/1960s/BHC_006_PH_202_verso.jpg')).toMatchObject({
      identifier: 'BHC_006_PH_202',
      part: 'verso',
    });
    expect(groupsOf(optional, 'Artifacts/scan0042.jpg')).toBeNull();
    expect(groupsOf(optional, '_scratch/notes.txt')).toBeNull();
  });

  it('reads a folder per object: the folder is the identifier and the file the page', () => {
    const example = withLabel(
      withLabel(withLabel({ path: 'Letters/BHC_001/003.tif', labels: {}, optional: [] }, 'd0', 'set'), 'd1', 'identifier'),
      'n0',
      'page',
    );
    const { pattern, shape } = compileExample(example);
    expect(shape).toBe('{set}/{identifier}/…/{page}.{ext}');
    expect(pattern).toBe('^(?<set>[^/]+)/(?<identifier>[^/]+)/(?:[^/]+/)*(?<page>[0-9]{3})\\.(?<ext>[A-Za-z0-9]+)$');
    expect(groupsOf(example, 'Letters/BHC_002/017.tif')).toEqual({
      set: 'Letters',
      identifier: 'BHC_002',
      page: '017',
      ext: 'tif',
    });
    expect(groupsOf(example, 'Letters/017.tif')).toBeNull();
  });

  it('keeps an unmarked folder above a marked one as one segment', () => {
    const example = withLabel({ path: 'Box 3/BHC_001/003.tif', labels: {}, optional: [] }, 'd1', 'identifier');
    expect(compileExample(example).shape).toBe('[Box 3]/{identifier}/…/[003].{ext}');
    expect(groupsOf(example, 'Box 4/BHC_009/001.jpg')).toMatchObject({ identifier: 'BHC_009' });
  });

  it('names a second identifier group identifier2, so the pattern compiles', () => {
    const example = withLabel(
      withLabel({ path: 'Letters/BHC_001/BHC_001_p1.tif', labels: {}, optional: [] }, 'd1', 'identifier'),
      'n0',
      'identifier',
    );
    expect(compileExample(example).pattern).toContain('(?<identifier2>[A-Z]+)');
    expect(() => new RegExp(compileExample(example).pattern)).not.toThrow();
  });
});

describe('defaultExample and the labels on offer', () => {
  it('starts with the whole name as the identifier and the top folder as the set', () => {
    const example = defaultExample(FRONT);
    expect(compileExample(example).shape).toBe('{set}/…/{identifier}.{ext}');
    expect(groupsOf(example, 'Artifacts/BHC_006_AF_001_back.jpg')).toMatchObject({
      identifier: 'BHC_006_AF_001_back',
    });
  });

  it('offers Item set on the top folder only, and Part on name parts only', () => {
    expect(labelsFor('d0')).toEqual(['set', 'identifier', 'page']);
    expect(labelsFor('d1')).toEqual(['identifier', 'page']);
    expect(labelsFor('n4')).toEqual(['identifier', 'part', 'page']);
  });

  it('lists a marked part after a delimiter as a candidate for optional', () => {
    expect(optionalCandidates(marked)).toEqual(['part']);
    expect(optionalCandidates(withOptional(marked, 'part', true))).toEqual([]);
  });
});
