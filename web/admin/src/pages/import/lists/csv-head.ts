/**
 * The head of a CSV the server wrote: its first records and how many records
 * it has, read the way RFC 4180 reads them (quoted cells may hold commas,
 * quotes and line breaks). Only for showing the list as it will download.
 */
export interface CsvHead {
  readonly rows: readonly (readonly string[])[];
  /** Every record, the header line included. */
  readonly count: number;
}

export const csvHead = (text: string, keep: number): CsvHead => {
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = '';
  let quoted = false;
  let count = 0;
  const start = text.startsWith('﻿') ? 1 : 0;
  const endRow = (): void => {
    row.push(cell);
    if (rows.length < keep) {
      rows.push(row);
    }
    count += 1;
    row = [];
    cell = '';
  };
  for (let at = start; at < text.length; at += 1) {
    const char = text.charAt(at);
    if (quoted) {
      if (char === '"' && text.charAt(at + 1) === '"') {
        cell += '"';
        at += 1;
      } else if (char === '"') {
        quoted = false;
      } else {
        cell += char;
      }
    } else if (char === '"') {
      quoted = true;
    } else if (char === ',') {
      row.push(cell);
      cell = '';
    } else if (char === '\r' && text.charAt(at + 1) === '\n') {
      at += 1;
      endRow();
    } else if (char === '\n') {
      endRow();
    } else {
      cell += char;
    }
  }
  if (cell !== '' || row.length > 0) {
    endRow();
  }
  return { rows, count };
};
