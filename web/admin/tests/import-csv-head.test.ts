/**
 * The head of a downloaded list: the first records as RFC 4180 reads them,
 * the byte order mark dropped, and every record counted.
 */
import { describe, expect, it } from 'vitest';
import { csvHead } from '../src/pages/import/lists/csv-head';

describe('csvHead', () => {
  it('keeps quoted commas, quotes and line breaks in one cell and counts every record', () => {
    const text = '﻿Identifier,Files,Title [dcterms:title]\r\nA,"a.jpg|b.jpg","Say ""hi"", then\r\nleave"\r\nB,b.jpg,\r\nC,c.jpg,Last';
    const head = csvHead(text, 2);
    expect(head.rows).toEqual([
      ['Identifier', 'Files', 'Title [dcterms:title]'],
      ['A', 'a.jpg|b.jpg', 'Say "hi", then\r\nleave'],
    ]);
    expect(head.count).toBe(4);
  });
});
