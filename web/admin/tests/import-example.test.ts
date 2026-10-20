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

const LETTERS = String.raw`[\p{L}\p{M}]+`;
const DIGITS = String.raw`\p{Nd}+`;
const EXT = String.raw`[\p{L}\p{M}\p{N}]+`;

/** The prototype's marks: top folder the set, `BHC_006_AF_001` the identifier, `front` the part. */
const marked: ImportExample = {
  path: FRONT,
  labels: { d0: 'set', n0: 'identifier', n2: 'identifier', n4: 'identifier', n6: 'identifier', n8: 'part' },
  optional: [],
};

const groupsOf = (example: ImportExample, path: string): Record<string, string> | null => {
  const match = new RegExp(compileExample(example).pattern, 'u').exec(path);
  return match === null ? null : { ...match.groups };
};

const identifierOf = (example: ImportExample, path: string): string | undefined =>
  groupsOf(example, path)?.identifier;

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
      String.raw`^(?<set>[^/]+)/(?:[^/]+/)*(?<identifier>${LETTERS}_${DIGITS}_${LETTERS}_${DIGITS})_(?<part>${LETTERS})\.(?<ext>${EXT})$`,
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
    expect(pattern).toBe(
      String.raw`^(?<set>[^/]+)/(?<identifier>[^/]+)/(?:[^/]+/)*(?<page>${DIGITS})\.(?<ext>${EXT})$`,
    );
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

  it('joins two marked folders into one identifier, so two objects stay two items', () => {
    const example: ImportExample = {
      path: 'Box 4/Folder 12/0001.tif',
      labels: { d0: 'identifier', d1: 'identifier', n0: 'page' },
      optional: [],
    };
    const { pattern, shape } = compileExample(example);
    expect(pattern.match(/\(\?<identifier>/g)).toHaveLength(1);
    expect(pattern).not.toContain('identifier2');
    expect(shape).toBe('{identifier}/…/{page}.{ext}');
    expect(identifierOf(example, 'Box 4/Folder 12/0001.tif')).toBe('Box 4/Folder 12');
    expect(identifierOf(example, 'Box 4/Folder 13/0001.tif')).toBe('Box 4/Folder 13');
    expect(identifierOf(example, 'Box 4/Folder 12/0001.tif')).not.toBe(
      identifierOf(example, 'Box 4/Folder 13/0001.tif'),
    );
  });

  it('joins a marked folder to a marked name part into one identifier', () => {
    const example: ImportExample = {
      path: 'Letters/BHC_001/BHC_001_p1.tif',
      labels: { d1: 'identifier', n0: 'identifier', n2: 'identifier', n4: 'part' },
      optional: [],
    };
    const { pattern } = compileExample(example);
    expect(pattern.match(/\(\?<identifier>/g)).toHaveLength(1);
    expect(pattern).not.toContain('identifier2');
    expect(identifierOf(example, 'Letters/BHC_001/BHC_001_p1.tif')).toBe('BHC_001/BHC_001');
    expect(identifierOf(example, 'Letters/BHC_002/BHC_002_p1.tif')).toBe('BHC_002/BHC_002');
  });
});

describe('every example matches its own path', () => {
  const paths = [
    'पत्र_001_आगे.jpg',
    'José_001.jpg',
    'Kraków_001.jpg',
    'LT_௧௨.jpg',
    'L_7.jpg',
    'LT_999.tif',
    '_front.jpg',
    'BHC_006_AF_001_front.jpg',
    'IMG_0042.JPG',
    'Box 3/Folder 12/0001.tif',
  ] as const;

  it.each(paths)('%s', (path) => {
    const example = defaultExample(path);
    const { pattern } = compileExample(example);
    const match = new RegExp(pattern, 'u').exec(path);
    expect(match, pattern).not.toBeNull();
    expect(match?.groups?.identifier).toBeTruthy();
  });
});

describe('the regex reads any script and any case', () => {
  it('takes one or more digits, not the example count', () => {
    const narrow = defaultExample('L_7.jpg');
    expect(identifierOf(narrow, 'L_12.jpg')).toBe('L_12');
    const wide = defaultExample('LT_999.tif');
    expect(identifierOf(wide, 'LT_1000.tif')).toBe('LT_1000');
  });

  it('takes digits from any script', () => {
    const tamil = defaultExample('LT_௧௨.jpg');
    expect(identifierOf(tamil, 'LT_௧௨.jpg')).toBe('LT_௧௨');
    expect(identifierOf(tamil, 'LT_123.jpg')).toBe('LT_123');
  });

  it('matches a letter part whatever its case', () => {
    const markedLower = { ...marked, path: 'Artifacts/BHC_006_AF_002_back.jpg' };
    expect(identifierOf(markedLower, 'Artifacts/bhc_006_af_003_BACK.JPG')).toBe('bhc_006_af_003');
    expect(identifierOf(defaultExample('_front.jpg'), '_Front.jpg')).toBe('Front');
    expect(identifierOf(defaultExample('José_001.jpg'), 'JOSÉ_001.jpg')).toBe('JOSÉ_001');
  });

  it('keeps combining marks inside a letter part', () => {
    const devanagari = defaultExample('पत्र_001_आगे.jpg');
    expect(identifierOf(devanagari, 'पत्र_002_आगे.jpg')).toBe('पत्र_002_आगे');
    expect(identifierOf(devanagari, 'पत्र.jpg')).toBeUndefined();
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
