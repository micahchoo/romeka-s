/**
 * What the lists say about one item or file, from facts the server computed:
 * the item's identifier, its title, and why it is left out.
 */
import type { LeftBy, RecordView } from '../../../api/import-model';
import type { FileRef } from '../../../api/import-plan';
import type { ImportMessageKey, ImportTranslator } from '../messages';

export const fileName = (file: FileRef): string => file.path.slice(file.path.lastIndexOf('/') + 1);

export const folderOf = (file: FileRef): string => {
  const slash = file.path.lastIndexOf('/');
  return slash < 0 ? '' : file.path.slice(0, slash);
};

/** The identifier the item gets, else its first file's name, else its row. */
export const recordName = (record: RecordView): string => {
  const key = record.resolved.itemKey;
  if (key !== null && key !== '') {
    return key;
  }
  const file = record.files[0];
  if (file !== undefined) {
    return fileName(file);
  }
  return record.rows[0]?.key ?? `#${String(record.id)}`;
};

export const recordTitle = (record: RecordView): string | null => record.slots['dcterms:title']?.[0]?.text ?? null;

const LEFT: Readonly<Record<Exclude<LeftBy, { readonly decision: number } | { readonly flag: string }>, ImportMessageKey>> = {
  parent: 'lists.left.parent',
  noFiles: 'lists.left.noFiles',
  identical: 'lists.left.identical',
  noRow: 'lists.left.noRow',
};

export const leftWords = (left: LeftBy, t: ImportTranslator['t']): string => {
  if (typeof left === 'string') {
    return t(LEFT[left]);
  }
  return 'decision' in left ? t('lists.left.you') : t('lists.left.problem', { problem: t(`problem.${left.flag}`) });
};
