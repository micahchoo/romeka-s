/**
 * The marked example as Simple makes it: a name split the way the server
 * splits it (`preview/names.rs`), one click that marks a part and the rest of
 * the name, and the pattern the shared `patternOf` writes from those marks.
 * The regex is matched here with JavaScript's engine, which reads these
 * patterns as the server's `regex` crate does.
 */
import { describe, expect, it } from 'vitest';
import { patternOf } from '../src/pages/import/example';
import { markOf, markedText, splitName, stemParts } from '../src/pages/import/simple/name-parts';

const key = (pattern: string, path: string): readonly [string, string | undefined] | null => {
  const groups = new RegExp(pattern, 'u').exec(path)?.groups;
  return groups?.['identifier'] === undefined ? null : [groups['identifier'], groups['part']];
};

describe('a file name in parts', () => {
  it('splits on _ - . and space, keeping the separator before each part, as the server does', () => {
    expect(splitName('BHC_006_AF-001 front.jpg')).toEqual([
      { text: 'BHC', sep: '' },
      { text: '006', sep: '_' },
      { text: 'AF', sep: '_' },
      { text: '001', sep: '-' },
      { text: 'front', sep: ' ' },
      { text: 'jpg', sep: '.' },
    ]);
    expect(splitName('a__b').map((part) => part.text)).toEqual(['a', '', 'b']);
  });

  it('offers every part but the ending, and a click covers the rest of the name', () => {
    expect(stemParts('letter 12 front.jpg').map((part) => part.text)).toEqual(['letter', '12', 'front']);
    expect(stemParts('README').map((part) => part.text)).toEqual(['README']);
    expect(markedText('diary/diary page 001.jpg', 1)).toBe('page 001');
  });
});

describe('the marks Simple makes', () => {
  it('reads the ticked words in the marked place as the part that changes', () => {
    const mark = markOf('letters/letter 12 front.jpg', 2, { kind: 'words', words: ['front', 'back', 'पुढे'] });
    expect(mark).toEqual({ kind: 'words', words: ['front', 'back', 'पुढे'], sep: ' ' });
    const pattern = patternOf([mark]);
    expect(key(pattern, 'letters/letter 12 front.jpg')).toEqual(['letter 12', 'front']);
    expect(key(pattern, 'पत्रे/पत्र 3 पुढे.jpg')).toEqual(['पत्र 3', 'पुढे']);
    expect(key(pattern, 'letters/letter 12 verso.jpg')).toEqual(['letter 12 verso', undefined]);
  });

  it('reads any number in place of "page 001", beside a word mark', () => {
    const pages = markOf('diary page 001.jpg', 1, { kind: 'number', words: [] });
    expect(pages).toEqual({ kind: 'number', text: 'page 001', sep: ' ' });
    const pattern = patternOf([markOf('letter 12 front.jpg', 2, { kind: 'words', words: ['front', 'back'] }), pages]);
    expect(key(pattern, 'diary/diary page 117.jpg')).toEqual(['diary', 'page 117']);
    expect(key(pattern, 'letter 12 back.jpg')).toEqual(['letter 12', 'back']);
  });

  it('cannot mark the first part: it is where the item’s name starts', () => {
    expect(() => markOf('front_12.jpg', 0, { kind: 'words', words: ['front'] })).toThrow();
  });
});

/** The identifier each name gets under `pattern`, in order; `null` for a name the pattern does not fit. */
const identifiers = (pattern: string, paths: readonly string[]): readonly (string | null)[] => paths.map((path) => key(pattern, path)?.[0] ?? null);

describe('a changing part glued to the number (A04)', () => {
  it('splits where a run of digits meets a run of letters, with nothing between the two parts', () => {
    expect(stemParts('BHC006_GMT_AF_010a.jpg')).toEqual([
      { text: 'BHC006', sep: '' },
      { text: 'GMT', sep: '_' },
      { text: 'AF', sep: '_' },
      { text: '010', sep: '_' },
      { text: 'a', sep: '' },
    ]);
    expect(stemParts('IMG_0042b.tif').map((part) => part.text)).toEqual(['IMG', '0042', 'b']);
    expect(stemParts('p12v.jpg').map((part) => part.text)).toEqual(['p12', 'v']);
    expect(stemParts('BHC006front.jpg').map((part) => part.text)).toEqual(['BHC006', 'front']);
  });

  it('reads any letters after the number: the views of one object are one item', () => {
    const mark = markOf('Artifacts/BHC006_GMT_AF_010a.jpg', 4, { kind: 'letters', words: [] });
    expect(mark).toEqual({ kind: 'letters', text: 'a', sep: '' });
    const names = ['010a', '010b', '010c', '011a', '011b'].map((code) => `Artifacts/BHC006_GMT_AF_${code}.jpg`);
    expect(identifiers(patternOf([mark]), names)).toEqual(['BHC006_GMT_AF_010', 'BHC006_GMT_AF_010', 'BHC006_GMT_AF_010', 'BHC006_GMT_AF_011', 'BHC006_GMT_AF_011']);
    expect(key(patternOf([mark]), 'Artifacts/BHC006_GMT_AF_010c.jpg')).toEqual(['BHC006_GMT_AF_010', 'c']);
  });

  it('takes letters only where they follow a number, so other names stay whole', () => {
    const pattern = patternOf([markOf('IMG_0042b.tif', 2, { kind: 'letters', words: [] })]);
    expect(identifiers(pattern, ['IMG_0042a.tif', 'IMG_0042b.tif', 'IMG_0043.tif', 'notes.txt', 'scan_front.jpg', 'Rebecca.jpg', 'BHC006(1).jpg'])).toEqual([
      'IMG_0042',
      'IMG_0042',
      'IMG_0043',
      'notes',
      'scan_front',
      'Rebecca',
      'BHC006(1)',
    ]);
  });

  it('reads only the ticked words after the number, as in folio 12 recto and verso', () => {
    const mark = markOf('p12v.jpg', 1, { kind: 'words', words: ['r', 'v'] });
    expect(mark).toEqual({ kind: 'words', words: ['r', 'v'], sep: '' });
    expect(identifiers(patternOf([mark]), ['p12r.jpg', 'p12v.jpg', 'p13r.jpg', 'p13x.jpg', 'Gustav.jpg'])).toEqual(['p12', 'p12', 'p13', 'p13x', 'Gustav']);
  });

  it('beside a mark after a separator, each mark still reads its own place', () => {
    const pattern = patternOf([
      markOf('IMG_0042b.tif', 2, { kind: 'letters', words: [] }),
      markOf('letter 12 front.jpg', 2, { kind: 'words', words: ['front', 'back'] }),
    ]);
    expect(identifiers(pattern, ['IMG_0042b.tif', 'letter 12 back.jpg', 'photo front.jpg'])).toEqual(['IMG_0042', 'letter 12', 'photo']);
  });

  it('leaves names with no letters after a number as they were', () => {
    expect(stemParts('scan_001.tif').map((part) => part.text)).toEqual(['scan', '001']);
    const mark = markOf('scan_001.tif', 1, { kind: 'number', words: [] });
    expect(mark).toEqual({ kind: 'number', text: '001', sep: '_' });
    expect(patternOf([mark])).toBe('^(?:[^/]*/)*(?<identifier>[^/]+?)(?:_(?<part>\\d+))?(?:\\.[^./]*)?$');
    expect(identifiers(patternOf([mark]), ['scan_001.tif', 'scan_002.tif'])).toEqual(['scan', 'scan']);
  });
});
