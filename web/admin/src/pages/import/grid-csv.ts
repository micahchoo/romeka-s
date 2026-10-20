/**
 * The grid as a CSV file in the shape `romeka import csv` reads: a header of
 * terms, the identifier first, then one line per row it is given.
 * Spreadsheet columns that map to no field are left out.
 */
import type { RowPreview, RowsPreview } from '../../api/import-preview';

const IDENTIFIER = 'dcterms:identifier';

const quote = (cell: string): string => (/[",\r\n]/.test(cell) ? `"${cell.replaceAll('"', '""')}"` : cell);

const line = (cells: readonly string[]): string => cells.map(quote).join(',');

export const gridCsv = (columns: RowsPreview['columns'], rows: readonly RowPreview[]): string => {
  const terms = columns.map((column) => column.term);
  const body = rows.map((row) => {
    const byTerm = new Map(row.cells.map((cell) => [cell.term, cell.value]));
    return line([row.identifier, ...terms.map((term) => byTerm.get(term) ?? '')]);
  });
  return [line([IDENTIFIER, ...terms]), ...body].join('\r\n') + '\r\n';
};

/** Hands `text` to the browser as a file download. */
export const downloadText = (name: string, text: string): void => {
  const url = URL.createObjectURL(new Blob([text], { type: 'text/csv;charset=utf-8' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = name;
  link.click();
  URL.revokeObjectURL(url);
};
