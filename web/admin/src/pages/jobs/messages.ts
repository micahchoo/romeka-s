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
