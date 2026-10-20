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
