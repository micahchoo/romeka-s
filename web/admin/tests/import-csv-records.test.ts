/**
 * The admin's one CSV reader, for every list it shows: the downloaded list
 * and the head of an uploaded one.
 */
import { describe, expect, it } from 'vitest';
import { readCsv } from '../src/pages/import/csv-records';

const cellsOf = (text: string, delimiter?: string): readonly (readonly string[])[] => readCsv(text, delimiter).records.map((record) => record.cells);

describe('readCsv', () => {
  it('keeps quoted commas, quotes and line breaks in one cell and counts every record', () => {
    const text = '﻿Identifier,Files,Title [dcterms:title]\r\nA,"a.jpg|b.jpg","Say ""hi"", then\r\nleave"\r\nB,b.jpg,\r\nC,c.jpg,Last';
    const read = readCsv(text, ',', 2);
    expect(read.records.map((record) => record.cells)).toEqual([
      ['Identifier', 'Files', 'Title [dcterms:title]'],
      ['A', 'a.jpg|b.jpg', 'Say "hi", then\r\nleave'],
    ]);
    expect(read.count).toBe(4);
  });

  it('drops the byte order mark only at the start', () => {
    expect(cellsOf('﻿Id\nA﻿')).toEqual([['Id'], ['A﻿']]);
  });

  it('ends a record at CRLF and LF, and keeps a lone CR in its cell', () => {
    expect(cellsOf('a,b\r\nc,d\ne\rf,g')).toEqual([
      ['a', 'b'],
      ['c', 'd'],
      ['e\rf', 'g'],
    ]);
  });

  it('reads the delimiter it is given, and no other', () => {
    expect(cellsOf('a;b,c\t"d;e"', ';')).toEqual([['a', 'b,c\t"d', 'e"']]);
    expect(cellsOf('a\tb;c\t"d\te"', '\t')).toEqual([['a', 'b;c', 'd\te']]);
    expect(cellsOf('a|"b|c"|', '|')).toEqual([['a', 'b|c', '']]);
  });

  it('opens a quote only at the start of a cell', () => {
    expect(cellsOf('5" disk,"x"')).toEqual([['5" disk', 'x']]);
  });

  it('gives each record the line it starts on', () => {
    const read = readCsv('Id,Note\r\nA,"one\ntwo"\n\nB,three\n');
    expect(read.records).toEqual([
      { line: 1, cells: ['Id', 'Note'] },
      { line: 2, cells: ['A', 'one\ntwo'] },
      { line: 4, cells: [''] },
      { line: 5, cells: ['B', 'three'] },
    ]);
    expect(read.count).toBe(4);
  });

  it('drops a record the head of a list leaves inside quotes', () => {
    expect(cellsOf('Id,Note\nA,"cut\nshort')).toEqual([['Id', 'Note']]);
  });

  it('reads an empty text as no record', () => {
    expect(readCsv('')).toEqual({ records: [], count: 0 });
  });
});
