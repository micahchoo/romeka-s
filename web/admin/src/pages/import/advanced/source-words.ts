/**
 * A field's source in words, and the sources a field can add: the columns
 * of each list, the folders, the file facts, the pattern's groups, the tags
 * and a constant. A choice's `<select>` value is its source as JSON, which
 * is unique, and the choice keeps the source itself.
 */
import { FILE_FACTS, TAG_IDS } from '../../../api/import-plan';
import type { FileFact, ImportPlan, SourceRef } from '../../../api/import-plan';
import type { SheetView } from '../../../api/import-sheet-views';
import type { AdvancedMessageKey, AdvancedTranslator } from './messages';
import { seriesOf } from '../series';

const FACT_WORDS: Readonly<Record<FileFact, AdvancedMessageKey>> = {
  name: 'adv.source.file.name',
  path: 'adv.source.file.path',
  size: 'adv.source.file.size',
  format: 'adv.source.file.format',
};

const sheetName = (plan: ImportPlan, sheet: string): string => seriesOf(plan).lists.find((one) => one.id === sheet)?.file ?? sheet;

export const sourceWords = (source: SourceRef, plan: ImportPlan, t: AdvancedTranslator['t']): string => {
  switch (source.from) {
    case 'column':
      return t('adv.source.column', { file: sheetName(plan, source.sheet), header: source.column.header });
    case 'folder':
      return source.level === 'path' ? t('adv.source.folderPath') : t('adv.source.folder', { level: source.level });
    case 'file':
      return t(FACT_WORDS[source.fact]);
    case 'group':
      return t('adv.source.group', { name: source.name });
    case 'tag':
      return source.files === 'first'
        ? t('adv.source.tagFirst', { tag: source.tag })
        : source.files === 'all'
          ? t('adv.source.tagAll', { tag: source.tag })
          : t('adv.source.tagPart', { tag: source.tag, part: source.files.part });
    case 'constant':
      return t('adv.source.constant', { value: source.value });
  }
};

export interface SourceChoice {
  readonly value: string;
  readonly label: string;
  readonly source: SourceRef;
}

const FOLDER_LEVELS = [1, 2, 3] as const;

/** Every source but a constant, which the person types. */
export const sourceChoices = (
  plan: ImportPlan,
  sheets: readonly SheetView[],
  groups: readonly string[],
  t: AdvancedTranslator['t'],
): readonly SourceChoice[] => {
  const choice = (source: SourceRef): SourceChoice => ({ value: JSON.stringify(source), label: sourceWords(source, plan, t), source });
  return [
    ...sheets.flatMap((sheet) =>
      (sheet.read?.columns ?? []).filter((column) => column.mappable).map((column) => choice({ from: 'column', sheet: sheet.id, column: { index: column.index, header: column.header } })),
    ),
    ...FOLDER_LEVELS.map((level) => choice({ from: 'folder', level })),
    choice({ from: 'folder', level: 'path' }),
    ...FILE_FACTS.map((fact) => choice({ from: 'file', fact })),
    ...groups.map((name) => choice({ from: 'group', name })),
    ...TAG_IDS.map((tag) => choice({ from: 'tag', tag, files: 'first' })),
  ];
};

export const choiceFor = (choices: readonly SourceChoice[], value: string): SourceRef | null =>
  choices.find((one) => one.value === value)?.source ?? null;
