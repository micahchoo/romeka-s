/**
 * The words a history row says about its version: who saved it, or why no one
 * is named, and what it changed against the version before it. Every count
 * comes from the server (`omeka_api::page_changes`); this only phrases them.
 */
import type { ChangedPageBlock, PageVersionSummary } from '../../../../api/types';
import type { Locale } from '../../../../i18n';
import type { BlockRegistry } from '../../blockSchema';
import type { Translate } from '../canvas/summary';
import { layoutLabel } from '../canvas/summary';
import { counted } from '../messages';
import { readLayoutData } from '../values';

export const authorText = (summary: PageVersionSummary, t: Translate): string => {
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

/** What the version changed, as one line; the oldest kept version says why it has no line. */
export const changesText = (
  summary: PageVersionSummary,
  registry: BlockRegistry,
  locale: Locale,
  t: Translate,
): string => {
  const { changes } = summary;
  if (changes === null) {
    return t(summary.version === 1 ? 'history.firstVersion' : 'history.olderGone');
  }
  const parts: string[] = [];
  if (changes.title !== null) {
    parts.push(t('history.retitled', changes.title));
  }
  if (changes.settingsChanged) {
    parts.push(t('history.settingsChanged'));
  }
  if (changes.added > 0) {
    parts.push(counted(t, 'blocksAdded', changes.added));
  }
  if (changes.removed > 0) {
    parts.push(counted(t, 'blocksRemoved', changes.removed));
  }
  if (changes.changed > 0) {
    const names = changes.changedBlocks.map((block) => blockName(registry, block, t));
    parts.push(
      names.length === 0
        ? counted(t, 'blocksChanged', changes.changed)
        : counted(t, 'blocksChangedNamed', changes.changed, {
            names: new Intl.ListFormat(locale, { type: 'conjunction' }).format(names),
          }),
    );
  }
  if (changes.moved > 0) {
    parts.push(counted(t, 'blocksMoved', changes.moved));
  }
  return parts.length === 0
    ? t('history.noChanges')
    : new Intl.ListFormat(locale, { type: 'unit', style: 'short' }).format(parts);
};
