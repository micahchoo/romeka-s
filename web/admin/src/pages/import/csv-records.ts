/**
 * The admin's one CSV reader: the records of a list's text, read the way
 * RFC 4180 reads them. A cell that starts with a double quote may hold the
 * delimiter, doubled quotes and line breaks; a leading byte order mark is
 * dropped; CRLF and LF end a record, and a lone CR is a character of its
 * cell. The text may be the head of a longer list, so a record the text
 * leaves inside quotes is cut short and dropped.
 */

/** One record: the line it starts on, from 1, and its cells. */
export interface CsvRecord {
  readonly line: number;
  readonly cells: readonly string[];
}

export interface CsvRead {
  /** The first `keep` records. */
  readonly records: readonly CsvRecord[];
  /** Every record, the header line included. */
  readonly count: number;
}

export const readCsv = (text: string, delimiter = ',', keep = Number.POSITIVE_INFINITY): CsvRead => {
  const records: CsvRecord[] = [];
  let cells: string[] = [];
  let cell = '';
  let quoted = false;
  let line = 1;
  let start = 1;
  let count = 0;
  const end = (): void => {
    cells.push(cell);
    if (records.length < keep) {
      records.push({ line: start, cells });
    }
    count += 1;
    cells = [];
    cell = '';
  };
  for (let at = text.startsWith('﻿') ? 1 : 0; at < text.length; at += 1) {
    const char = text.charAt(at);
    if (char === '\n') {
      line += 1;
    }
    if (quoted) {
      if (char === '"' && text.charAt(at + 1) === '"') {
        cell += '"';
        at += 1;
      } else if (char === '"') {
        quoted = false;
      } else {
        cell += char;
      }
    } else if (char === '"' && cell === '') {
      quoted = true;
    } else if (char === delimiter) {
      cells.push(cell);
      cell = '';
    } else if (char === '\r' && text.charAt(at + 1) === '\n') {
      at += 1;
      line += 1;
      end();
      start = line;
    } else if (char === '\n') {
      end();
      start = line;
    } else {
      cell += char;
    }
  }
  if (!quoted && (cell !== '' || cells.length > 0)) {
    end();
  }
  return { records, count };
};
