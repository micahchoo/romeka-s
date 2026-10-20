/**
 * Every row a rows query selects, read page by page from the preview. A bulk
 * fix needs the identifiers of all the rows with one problem, and a CSV
 * download needs every row and the columns, but the server answers at most
 * 200 rows a page.
 */
import { api } from '../../api/client';
import type { ProblemKind, RowPreview, RowsPreview } from '../../api/import-preview';
import { previewImport } from '../../api/imports';
import type { ImportId, ImportPlan } from '../../api/imports';

const PAGE = 200;

export interface RowSelection {
  readonly kind?: ProblemKind;
  readonly term?: string;
}

export const collectRows = async (
  id: ImportId,
  plan: ImportPlan,
  selection: RowSelection,
): Promise<{ readonly columns: RowsPreview['columns']; readonly rows: readonly RowPreview[] }> => {
  const rows: RowPreview[] = [];
  for (;;) {
    const preview = await previewImport(api, id, {
      include: ['rows'],
      rows: { problemsOnly: false, ...selection, offset: rows.length, limit: PAGE },
      plan,
    });
    const page = preview.rows?.page ?? [];
    rows.push(...page);
    if (page.length === 0 || rows.length >= (preview.rows?.filtered ?? 0)) {
      return { columns: preview.rows?.columns ?? [], rows };
    }
  }
};
