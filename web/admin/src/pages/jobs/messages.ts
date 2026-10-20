/**
 * The Jobs page's status labels, read through the `admin` namespace
 * (`src/i18n/namespaces/admin/`).
 */
import { useMemo } from 'react';
import { useTranslator } from '../../i18n';
import { admin } from '../../i18n/namespaces/admin';

/**
 * `JobRepresentation::statusLabel` through `@translate`: the label of a stored
 * status in the current locale, or `null` for a status upstream has no label
 * for, which the caller prints as unknown.
 */
export const useJobStatusLabel = (): ((status: string | null) => string | null) => {
  const { t } = useTranslator(admin);
  return useMemo(
    () =>
      (status: string | null): string | null => {
        // `JobRepresentation::$statusLabels`, keyed by the stored `Job::STATUS_*`.
        const key = `jobs.status.${status ?? ''}`;
        return admin.has(key) ? t(key) : null;
      },
    [t],
  );
};

/** Romeka's own job classes (`docs/archive-import.md`, Jobs) and the name each reads as. */
const CLASS_TEXT: Readonly<Record<string, 'jobs.class.importArchive' | 'jobs.class.undoImport'>> = {
  'Romeka\\Job\\ImportArchive': 'jobs.class.importArchive',
  'Romeka\\Job\\UndoImport': 'jobs.class.undoImport',
};

/** The archive import a job works on, from its `{ "importId": n }` args, or `null`. */
export const importOfJob = (jobClass: string, args: unknown): number | null => {
  if (!(jobClass in CLASS_TEXT) || typeof args !== 'object' || args === null || !('importId' in args)) {
    return null;
  }
  const id = args.importId;
  return typeof id === 'number' && Number.isSafeInteger(id) && id > 0 ? id : null;
};

/** A job class as a reader names it: Romeka's own get a name, any other shows as stored. */
export const useJobClassLabel = (): ((jobClass: string) => string) => {
  const { t } = useTranslator(admin);
  return useMemo(
    () =>
      (jobClass: string): string => {
        const key = CLASS_TEXT[jobClass];
        return key === undefined ? jobClass : t(key);
      },
    [t],
  );
};
