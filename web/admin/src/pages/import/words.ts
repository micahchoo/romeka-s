/**
 * The plain words for an import that every screen shares: where its files
 * are, and how far it got.
 */
import type { SourcePlan } from '../../api/import-plan';
import type { SourceState } from '../../api/import-sections';
import type { ImportRow } from '../../api/import-view';
import type { ImportMessageKey, ImportTranslator } from './messages';

/**
 * A staging source the last look found only lists in: an uploaded list lands
 * in staging too, but it is no files source.
 */
const holdsListsOnly = (source: SourcePlan, states: readonly SourceState[]): boolean => {
  const state = states.find((one) => one.source === source.id);
  return source.kind === 'staging' && state !== undefined && state.files === 0 && state.sheetFiles.length > 0;
};

const sourceWords = (source: SourcePlan, t: ImportTranslator['t'], states: readonly SourceState[]): string => {
  if (source.kind === 'staging') {
    return holdsListsOnly(source, states) ? t('words.computerLists') : t('words.computer');
  }
  return source.path === '' ? t('words.root', { root: source.root }) : t('words.folder', { path: source.path, root: source.root });
};

/**
 * "letters/1890 (archive)", or every source of the import, comma-separated.
 * With the last look's `states`, a staging source that holds only lists is
 * left out beside a files source, and named for its lists when it is alone.
 */
export const whereWords = (sources: readonly SourcePlan[], t: ImportTranslator['t'], states: readonly SourceState[] = []): string => {
  if (sources.length === 0) {
    return t('words.none');
  }
  const named = sources.filter((source) => !holdsListsOnly(source, states));
  return (named.length === 0 ? sources : named).map((source) => sourceWords(source, t, states)).join(', ');
};

/** The status as a person reads it; a run told to stop says so until it has. */
export const statusKey = (row: Pick<ImportRow, 'status' | 'stopRequested'>): ImportMessageKey =>
  row.status === 'running' && row.stopRequested ? 'status.stopping' : `status.${row.status}`;

/** Whether the run may go again: it stopped, or some records failed (`docs/import-v2.md`, "Retry"). */
export const canRetry = (row: Pick<ImportRow, 'status' | 'summary'>): boolean =>
  row.status === 'run_failed' || (row.status === 'done' && (row.summary?.failures.length ?? 0) > 0);
