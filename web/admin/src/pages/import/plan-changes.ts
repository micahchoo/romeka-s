/**
 * The plan edits the Sources and Grid steps make. Each takes a plan and
 * answers a new one, so a step hands it to `onChange` as it is; the server
 * derives everything the edit changes on the next preview.
 */
import type { CsvPlan, ImportPlan, MetaRule } from '../../api/imports';

export type Delimiter = CsvPlan['delimiter'];
export const DELIMITERS: readonly Delimiter[] = [',', ';', '\t'];

/** The plan's delimiter for a delimiter the server detected, if it is one the plan can hold. */
export const asDelimiter = (value: string): Delimiter | null =>
  DELIMITERS.find((entry) => entry === value) ?? null;

/** A spreadsheet with nothing chosen yet; the step adopts what the server detects. */
export const newCsv = (file: string): CsvPlan => ({
  file,
  delimiter: ',',
  headerRow: 1,
  windows1252: false,
  trim: false,
  joinColumn: '',
  columns: {},
});

export const withCsv = (plan: ImportPlan, change: (csv: CsvPlan) => CsvPlan): ImportPlan =>
  plan.csv === null ? plan : { ...plan, csv: change(plan.csv) };

export const withColumn = (
  plan: ImportPlan,
  header: string,
  target: CsvPlan['columns'][string],
): ImportPlan => withCsv(plan, (csv) => ({ ...csv, columns: { ...csv.columns, [header]: target } }));

/** The tag's rule replaced, or removed with `null`. One rule per tag. */
export const withMetaRule = (plan: ImportPlan, tag: string, rule: MetaRule | null): ImportPlan => {
  if (rule === null) {
    return { ...plan, meta: plan.meta.filter((entry) => entry.tag !== tag) };
  }
  const exists = plan.meta.some((entry) => entry.tag === tag);
  return { ...plan, meta: exists ? plan.meta.map((entry) => (entry.tag === tag ? rule : entry)) : [...plan.meta, rule] };
};

/** `values` edits, identifier to value, on one term. */
export const withEdits = (plan: ImportPlan, term: string, values: ReadonlyMap<string, string>): ImportPlan => {
  const edits = { ...plan.edits };
  for (const [identifier, value] of values) {
    edits[identifier] = { ...edits[identifier], [term]: value };
  }
  return { ...plan, edits };
};

export const withEdit = (plan: ImportPlan, identifier: string, term: string, value: string): ImportPlan =>
  withEdits(plan, term, new Map([[identifier, value]]));

/** The cell back to what the spreadsheet and the files give. */
export const withoutEdit = (plan: ImportPlan, identifier: string, term: string): ImportPlan => {
  const mine = Object.entries(plan.edits[identifier] ?? {}).filter(([key]) => key !== term);
  const others = Object.entries(plan.edits).filter(([key]) => key !== identifier);
  return {
    ...plan,
    edits: Object.fromEntries(mine.length === 0 ? others : [...others, [identifier, Object.fromEntries(mine)]]),
  };
};

const union = (list: readonly string[], add: Iterable<string>): readonly string[] => [...new Set([...list, ...add])];

export const withSkipped = (plan: ImportPlan, identifiers: Iterable<string>): ImportPlan => ({
  ...plan,
  skip: union(plan.skip, identifiers),
});

export const withIncluded = (plan: ImportPlan, identifier: string): ImportPlan => ({
  ...plan,
  skip: plan.skip.filter((entry) => entry !== identifier),
});

export const withoutFiles = (plan: ImportPlan, identifiers: Iterable<string>): ImportPlan => ({
  ...plan,
  withoutFiles: union(plan.withoutFiles, identifiers),
});

export const withCsvRows = (
  plan: ImportPlan,
  identifiers: Iterable<string>,
  choice: 'keep' | 'ignore',
): ImportPlan => ({
  ...plan,
  csvRow: { ...plan.csvRow, ...Object.fromEntries([...identifiers].map((identifier) => [identifier, choice])) },
});

/** The fill text for one row: `{identifier}` becomes the row's identifier. */
export const fillFor = (template: string, identifier: string): string =>
  template.replaceAll('{identifier}', identifier);
