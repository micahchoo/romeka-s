/**
 * How the grid's problems and value sources read: one message per problem
 * kind for the list of kinds, one short name per cell problem for a red cell,
 * and one line per source a column or a cell takes its value from.
 */
import type { ProblemKind, RowCell } from '../../api/import-preview';
import type { ImportMessageKey, ImportTranslator } from './messages';

export const KIND_TEXT: Readonly<Record<ProblemKind, ImportMessageKey>> = {
  empty: 'kind.empty',
  date: 'kind.date',
  space: 'kind.space',
  encoding: 'kind.encoding',
  cells: 'kind.cells',
  nofile: 'kind.nofile',
};

/** The cell kinds; `cells` and `nofile` belong to the row and show in the Problems column. */
export const CELL_PROBLEM: Readonly<Partial<Record<ProblemKind, ImportMessageKey>>> = {
  empty: 'problem.empty',
  date: 'problem.date',
  space: 'problem.space',
  encoding: 'problem.encoding',
};

const CELL_SOURCE: Readonly<Record<RowCell['source'], ImportMessageKey | null>> = {
  edit: 'source.edit',
  csv: 'source.fromCsv',
  file: 'source.fromFile',
  name: 'source.fromName',
  '': null,
};

export const cellSource = ({ t }: ImportTranslator, source: RowCell['source']): string => {
  const key = CELL_SOURCE[source];
  return key === null ? '' : t(key);
};

/** A column source as the server names it: `type`, `csv:<header>` or `tag:<tag>`. */
export const columnSource = ({ t }: ImportTranslator, source: string): string => {
  if (source.startsWith('csv:')) {
    return t('source.csv', { name: source.slice('csv:'.length) });
  }
  if (source.startsWith('tag:')) {
    return t('source.tag', { name: source.slice('tag:'.length) });
  }
  return source === 'type' ? t('source.type') : source;
};
