/**
 * The plain words for an import that every screen shares: where its files
 * are, and how far it got.
 */
import type { SourcePlan } from '../../api/import-plan';
import type { ImportRow } from '../../api/import-view';
import type { ImportMessageKey, ImportTranslator } from './messages';

const sourceWords = (source: SourcePlan, t: ImportTranslator['t']): string => {
  if (source.kind === 'staging') {
    return t('words.computer');
  }
  return source.path === '' ? t('words.root', { root: source.root }) : t('words.folder', { path: source.path, root: source.root });
};

/** "letters/1890 (archive)", or every source of the import, comma-separated. */
export const whereWords = (sources: readonly SourcePlan[], t: ImportTranslator['t']): string =>
  sources.length === 0 ? t('words.none') : sources.map((source) => sourceWords(source, t)).join(', ');

/** The status as a person reads it; a run told to stop says so until it has. */
export const statusKey = (row: Pick<ImportRow, 'status' | 'stopRequested'>): ImportMessageKey =>
  row.status === 'running' && row.stopRequested ? 'status.stopping' : `status.${row.status}`;

/** Whether the run may go again: it stopped, or some records failed (`docs/import-v2.md`, "Retry"). */
export const canRetry = (row: Pick<ImportRow, 'status' | 'summary'>): boolean =>
  row.status === 'run_failed' || (row.status === 'done' && (row.summary?.failures.length ?? 0) > 0);
