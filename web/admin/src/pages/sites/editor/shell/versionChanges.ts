/**
 * The words a history row says about its version: who saved it, or why no one
 * is named, and what it changed against the version before it. Every count
 * comes from the server (`omeka_api::page_changes`); this only phrases them.
 */
import type { ChangedPageBlock, PageVersionOrigin, PageVersionSummary } from '../../../../api/types';
import type { KeyOf, Locale, MessageParams } from '../../../../i18n';
import type { pageHistory } from '../../../../i18n/namespaces/page-history';
import type { BlockRegistry } from '../../blockSchema';
import type { Translate } from '../canvas/summary';
import { layoutLabel } from '../canvas/summary';
import { readLayoutData } from '../values';

/** The lookup the history words go through: the editor's card labels and `page-history`. */
export type HistoryTranslate = (
  key: Parameters<Translate>[0] | KeyOf<typeof pageHistory>,
  params?: MessageParams,
) => string;

/**
 * Each change count's singular and plural ids. The catalogue has no plural rules, so a count of
 * exactly one reads the singular message, the way every shipped locale writes "1 block".
 */
const COUNTED = {
  added: { one: 'history.addedOne', other: 'history.added' },
  removed: { one: 'history.removedOne', other: 'history.removed' },
  changed: { one: 'history.changedOne', other: 'history.changed' },
  changedNamed: { one: 'history.changedNamedOne', other: 'history.changedNamed' },
  moved: { one: 'history.movedOne', other: 'history.moved' },
} as const satisfies Readonly<Record<string, { one: KeyOf<typeof pageHistory>; other: KeyOf<typeof pageHistory> }>>;

const counted = (
  t: HistoryTranslate,
  noun: keyof typeof COUNTED,
  count: number,
  params: MessageParams = {},
): string => {
  const forms = COUNTED[noun];
  return t(count === 1 ? forms.one : forms.other, { ...params, count });
};

export const authorText = (summary: PageVersionSummary, t: HistoryTranslate): string => {
  switch (summary.author.kind) {
    case 'user':
      return summary.author.user.name;
    case 'notRecorded':
      return t('history.notRecorded');
    case 'deletedUser':
      return t('history.deletedUser');
  }
};

const blockName = (registry: BlockRegistry, block: ChangedPageBlock, t: Translate): string =>
  layoutLabel(registry, { layout: block.layout, layoutData: readLayoutData(block.layoutData) }, t);

/**
 * Why the oldest kept version has nothing to compare against: the page began
 * there (created, or history began), or the versions before it are gone.
 */
const OLDEST_TEXT = {
  create: 'history.created',
  beforeHistory: 'history.firstVersion',
  save: 'history.olderGone',
} as const satisfies Readonly<Record<PageVersionOrigin, string>>;

/** What the version changed, as one line; the oldest kept version says why it has no line. */
export const changesText = (
  summary: PageVersionSummary,
  registry: BlockRegistry,
  locale: Locale,
  t: HistoryTranslate,
): string => {
  const { changes } = summary;
  if (changes === null) {
    return t(OLDEST_TEXT[summary.origin]);
  }
  const parts: string[] = [];
  if (changes.title !== null) {
    parts.push(t('history.retitled', changes.title));
  }
  if (changes.settingsChanged) {
    parts.push(t('history.settingsChanged'));
  }
  if (changes.added > 0) {
    parts.push(counted(t, 'added', changes.added));
  }
  if (changes.removed > 0) {
    parts.push(counted(t, 'removed', changes.removed));
  }
  if (changes.changed > 0) {
    const names = changes.changedBlocks.map((block) => blockName(registry, block, t));
    parts.push(
      names.length === 0
        ? counted(t, 'changed', changes.changed)
        : counted(t, 'changedNamed', changes.changed, {
            names: new Intl.ListFormat(locale, { type: 'conjunction' }).format(names),
          }),
    );
  }
  if (changes.moved > 0) {
    parts.push(counted(t, 'moved', changes.moved));
  }
  return parts.length === 0
    ? t('history.noChanges')
    : new Intl.ListFormat(locale, { type: 'unit', style: 'short' }).format(parts);
};
