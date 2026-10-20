/**
 * How a label or a pattern group reads on screen: its name, its help line and
 * its colour class. A numbered group (`part2`) reads as its label.
 */
import type { Label } from '../../api/imports';
import type { ImportMessageKey } from './messages';

export const LABEL_TEXT: Readonly<Record<Label, { readonly name: ImportMessageKey; readonly help: ImportMessageKey }>> = {
  identifier: { name: 'label.identifier', help: 'help.identifier' },
  part: { name: 'label.part', help: 'help.part' },
  page: { name: 'label.page', help: 'help.page' },
  set: { name: 'label.set', help: 'help.set' },
};

const GROUP_TITLE: Readonly<Record<string, ImportMessageKey>> = {
  identifier: 'label.identifier',
  part: 'label.part',
  page: 'label.page',
  set: 'label.set',
  ext: 'label.ext',
};

const baseOf = (group: string): string => group.replace(/[0-9]+$/, '');

/** The column heading's message for a group, or `null` for a group of the user's own regex. */
export const groupTitleKey = (group: string): ImportMessageKey | null => GROUP_TITLE[baseOf(group)] ?? null;

export const tagClass = (group: string): string =>
  `import-tag import-tag-${groupTitleKey(group) === null ? 'other' : baseOf(group)}`;
