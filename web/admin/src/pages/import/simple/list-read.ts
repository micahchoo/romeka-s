/**
 * Reading a list file of the folder into the plan, as "Yes, use it" does. A
 * list in the folder is used from the start, as the suggested answer to "Is
 * it about these items?", so the groups on "What we found" can already say
 * which items have a row; "No" takes it out and keeps it out.
 */
import type { ImportId, ImportPlan, SheetPlan } from '../../../api/import-plan';
import { importApi } from '../../../api/import-routes';
import type { SourceState } from '../../../api/import-sections';
import { loadProperties } from '../lists/catalog';
import { declines, readState } from './example-state';
import { sheetOf, withRead } from './list-plan';

export interface ListFile {
  readonly source: string;
  readonly file: string;
}

/** The plan's list from a folder, which "Your list" talks about. */
export const folderSheet = (plan: ImportPlan): SheetPlan | undefined =>
  plan.sheets.find((sheet) => plan.sources.some((source) => source.id === sheet.source && source.kind === 'folder'));

/** The list files of the folders that the plan does not read. */
export const listFiles = (plan: ImportPlan, sources: readonly SourceState[]): readonly ListFile[] =>
  sources.flatMap((source) =>
    source.sheetFiles
      .filter((file) => !plan.sheets.some((sheet) => sheet.source === source.source && sheet.file === file))
      .map((file) => ({ source: source.source, file })),
  );

/** Those the person did not turn down. */
export const offered = (plan: ImportPlan, sources: readonly SourceState[]): readonly ListFile[] => {
  const { notLists } = readState(plan);
  return listFiles(plan, sources).filter((list) => !notLists.some((entry) => declines(entry, list.source, list.file)));
};

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
