/**
 * Reading a list file of the folder into the plan, as "Use this list" does.
 * The list that pairs the most rows is used from the start, as the suggested
 * answer, so the groups on "What we found" can already say which items have
 * a row; "Not about these items" keeps a list out.
 */
import type { ImportId, ImportPlan, SheetPlan } from '../../../api/import-plan';
import { importApi } from '../../../api/import-routes';
import { loadProperties } from '../lists/catalog';
import { sheetOf, withRead } from './list-plan';

export interface ListFile {
  readonly source: string;
  readonly file: string;
}

/** The plan's lists from a folder, which "Your list" talks about. */
export const folderSheets = (plan: ImportPlan): readonly SheetPlan[] =>
  plan.sheets.filter((sheet) => plan.sources.some((source) => source.id === sheet.source && source.kind === 'folder'));

/**
 * The edit that adds `list` to a plan: read once with automatic options, and
 * once more on the header line the server guessed when that is not line 1.
 */
export const listEdit = async (id: ImportId, plan: ImportPlan, list: ListFile): Promise<(now: ImportPlan) => ImportPlan> => {
  const readWith = (sheet: SheetPlan) => importApi.preview(id, ['sheets', 'columnMatches'], { plan: { ...plan, sheets: [...plan.sheets, sheet] } });
  const first = sheetOf(plan, list.source, list.file);
  let sheet = first;
  let preview = await readWith(first);
  const line = preview.sheets.find((one) => one.id === first.id)?.guess?.headerLine ?? 1;
  if (line !== first.read.headerLine) {
    sheet = { ...first, read: { ...first.read, headerLine: line } };
    preview = await readWith(sheet);
  }
  const fields = await loadProperties();
  const view = preview.sheets.find((one) => one.id === sheet.id);
  const read = view?.read ?? null;
  if (read === null) {
    throw new Error(view?.error ?? list.file);
  }
  const matches = preview.columnMatches.find((one) => one.sheet === sheet.id);
  return (now) => (now.sheets.some((one) => one.file === sheet.file && one.source === sheet.source) ? now : withRead({ ...now, sheets: [...now.sheets, sheet] }, sheet.id, read, matches, fields));
};
