/**
 * A change and an item's fields in plain words, for the preview's before and after, and for the
 * recent changes list. Names come from the browse option lists; a row the lists lack reads `#id`.
 */
import type { BulkEdit, BulkFields } from '../../../api/bulk';
import type { ClassId } from '../../../api/types';
import type { KeyOf, TermLabels, Translator } from '../../../i18n';
import type { bulk } from '../../../i18n/namespaces/bulk';
import type { BrowseOptions } from '../browse-options';

export type BulkTranslate = Translator<KeyOf<typeof bulk>>['t'];

export interface Namer {
  readonly t: BulkTranslate;
  readonly options: BrowseOptions;
  readonly text: TermLabels;
}

const nameIn = <K extends number>(map: ReadonlyMap<K, string>, id: K): string => map.get(id) ?? `#${String(id)}`;

const namesIn = <K extends number>(map: ReadonlyMap<K, string>, ids: readonly K[]): string =>
  ids.map((id) => nameIn(map, id)).join(', ');

export const classNameOf = ({ options, text }: Namer, id: ClassId): string => {
  const name = options.classNames.get(id);
  return name === undefined ? `#${String(id)}` : text.label(name.term, name.label);
};

const sameIds = (a: readonly number[], b: readonly number[]): boolean =>
  a.length === b.length && a.every((id, at) => id === b[at]);

/**
 * The fields that differ between `before` and `after`, each in words, one list for each side.
 * A field the change leaves alone is not named.
 */
export const fieldWords = (
  namer: Namer,
  before: BulkFields,
  after: BulkFields,
): { readonly before: readonly string[]; readonly after: readonly string[] } => {
  const { t, options } = namer;
  const sides: [string[], string[]] = [[], []];
  const each = (fields: BulkFields, side: 0 | 1): void => {
    const out = sides[side];
    if (before.isPublic !== after.isPublic) {
      out.push(fields.isPublic ? t('bulk.word.public') : t('bulk.word.private'));
    }
    if (before.owner !== after.owner) {
      out.push(fields.owner === null ? t('bulk.word.noOwner') : t('bulk.word.owner', { name: nameIn(options.users, fields.owner) }));
    }
    if (before.class !== after.class) {
      out.push(fields.class === null ? t('bulk.word.noClass') : t('bulk.word.class', { name: classNameOf(namer, fields.class) }));
    }
    if (before.template !== after.template) {
      out.push(
        fields.template === null
          ? t('bulk.word.noTemplate')
          : t('bulk.word.template', { name: nameIn(options.templates, fields.template) }),
      );
    }
    if (!sameIds(before.itemSets, after.itemSets)) {
      out.push(
        fields.itemSets.length === 0 ? t('bulk.word.noSets') : t('bulk.word.sets', { names: namesIn(options.itemSets, fields.itemSets) }),
      );
    }
    if (!sameIds(before.sites, after.sites)) {
      out.push(fields.sites.length === 0 ? t('bulk.word.noSites') : t('bulk.word.sites', { names: namesIn(options.sites, fields.sites) }));
    }
  };
  each(before, 0);
  each(after, 1);
  return { before: sides[0], after: sides[1] };
};

/** What a change does, in words, joined: "Private; Add to Letters". */
export const editWords = (namer: Namer, edit: BulkEdit): string => {
  const { t, options } = namer;
  const parts: string[] = [];
  if (edit.isPublic !== null) {
    parts.push(edit.isPublic ? t('bulk.word.public') : t('bulk.word.private'));
  }
  if (edit.owner !== null) {
    parts.push(t('bulk.word.owner', { name: nameIn(options.users, edit.owner) }));
  }
  if (edit.class !== null) {
    parts.push(edit.class.kind === 'clear' ? t('bulk.word.noClass') : t('bulk.word.class', { name: classNameOf(namer, edit.class.id) }));
  }
  if (edit.template !== null) {
    parts.push(
      edit.template.kind === 'clear'
        ? t('bulk.word.noTemplate')
        : t('bulk.word.template', { name: nameIn(options.templates, edit.template.id) }),
    );
  }
  const memberships = [
    [edit.itemSets.add, 'bulk.word.addSets', options.itemSets],
    [edit.itemSets.remove, 'bulk.word.removeSets', options.itemSets],
    [edit.sites.add, 'bulk.word.addSites', options.sites],
    [edit.sites.remove, 'bulk.word.removeSites', options.sites],
  ] as const;
  for (const [ids, key, map] of memberships) {
    if (ids.length > 0) {
      parts.push(t(key, { names: namesIn<number>(map, ids) }));
    }
  }
  return parts.join('; ');
};
