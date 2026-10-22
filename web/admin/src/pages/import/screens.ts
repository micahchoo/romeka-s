/**
 * The screens of one import, in both modes, and the addresses that name them:
 * `#/import/<id>/<mode>/<screen>`. Simple and Advanced are two views of one
 * plan, so every screen names its counterpart in the other mode, and the mode
 * switch lands there (`docs/import-v2.md`, "Simple and Advanced").
 *
 * A screen without a `step` is a sub-screen: the step bar or the side index
 * marks its `parent` as the place the person is.
 */
import type { ImportDetail } from '../../api/import-view';
import type { ImportId } from '../../api/import-plan';
import type { ImportMessageKey } from './messages';

export const SIMPLE_SCREENS = [
  'files',
  'structure',
  'found',
  'example',
  'all-files',
  'list',
  'pair',
  'where',
  'check',
  'all-items',
  'add',
] as const;
export const ADVANCED_SCREENS = [
  'files',
  'lists',
  'fields',
  'problems',
  'existing',
  'changes',
  'items',
  'run',
  'all-files',
  'pair',
] as const;

export type SimpleScreen = (typeof SIMPLE_SCREENS)[number];
export type AdvancedScreen = (typeof ADVANCED_SCREENS)[number];
export type Mode = 'simple' | 'advanced';

export type Place =
  | { readonly mode: 'simple'; readonly screen: SimpleScreen }
  | { readonly mode: 'advanced'; readonly screen: AdvancedScreen };

interface Screen<Own, Other> {
  /** The heading over the screen. */
  readonly title: ImportMessageKey;
  /** The name in the step bar or the side index; a sub-screen has none. */
  readonly step?: ImportMessageKey;
  readonly parent?: Own;
  /** The screen of the other mode that shows the same thing. */
  readonly other: Other;
}

export const SIMPLE: Readonly<Record<SimpleScreen, Screen<SimpleScreen, AdvancedScreen>>> = {
  files: { title: 'simple.files.title', step: 'simple.files.step', other: 'files' },
  // Reached by its address only until Simple's steps become Structure, Check and Add.
  structure: { title: 'simple.structure.title', parent: 'found', other: 'files' },
  found: { title: 'simple.found.title', step: 'simple.found.step', other: 'files' },
  example: { title: 'simple.example.title', parent: 'found', other: 'files' },
  'all-files': { title: 'simple.allFiles.title', parent: 'found', other: 'all-files' },
  list: { title: 'simple.list.title', step: 'simple.list.step', other: 'lists' },
  pair: { title: 'simple.pair.title', parent: 'list', other: 'pair' },
  where: { title: 'simple.where.title', step: 'simple.where.step', other: 'fields' },
  check: { title: 'simple.check.title', step: 'simple.check.step', other: 'problems' },
  'all-items': { title: 'simple.allItems.title', parent: 'check', other: 'items' },
  add: { title: 'simple.add.title', step: 'simple.add.step', other: 'run' },
};

export const ADVANCED: Readonly<Record<AdvancedScreen, Screen<AdvancedScreen, SimpleScreen>>> = {
  files: { title: 'advanced.files.title', step: 'advanced.files.step', other: 'found' },
  lists: { title: 'advanced.lists.title', step: 'advanced.lists.step', other: 'list' },
  fields: { title: 'advanced.fields.title', step: 'advanced.fields.step', other: 'where' },
  problems: { title: 'advanced.problems.title', step: 'advanced.problems.step', other: 'check' },
  existing: { title: 'advanced.existing.title', step: 'advanced.existing.step', other: 'add' },
  changes: { title: 'advanced.changes.title', step: 'advanced.changes.step', other: 'check' },
  items: { title: 'advanced.items.title', step: 'advanced.items.step', other: 'all-items' },
  run: { title: 'advanced.run.title', step: 'advanced.run.step', other: 'add' },
  'all-files': { title: 'advanced.allFiles.title', parent: 'files', other: 'all-files' },
  pair: { title: 'advanced.pair.title', parent: 'lists', other: 'pair' },
};

/** The step bar's entries (Simple) or the side index's (Advanced), in order, and which one is current. */
export const stepsOf = (place: Place): { readonly steps: readonly Place[]; readonly current: Place } => {
  if (place.mode === 'simple') {
    const screen = SIMPLE[place.screen];
    return {
      steps: SIMPLE_SCREENS.filter((id) => SIMPLE[id].step !== undefined).map((id) => ({ mode: 'simple', screen: id })),
      current: { mode: 'simple', screen: screen.parent ?? place.screen },
    };
  }
  const screen = ADVANCED[place.screen];
  return {
    steps: ADVANCED_SCREENS.filter((id) => ADVANCED[id].step !== undefined).map((id) => ({ mode: 'advanced', screen: id })),
    current: { mode: 'advanced', screen: screen.parent ?? place.screen },
  };
};

export const screenOf = (place: Place): Screen<string, string> =>
  place.mode === 'simple' ? SIMPLE[place.screen] : ADVANCED[place.screen];

export const counterpart = (place: Place): Place =>
  place.mode === 'simple'
    ? { mode: 'advanced', screen: SIMPLE[place.screen].other }
    : { mode: 'simple', screen: ADVANCED[place.screen].other };

export const samePlace = (a: Place, b: Place): boolean => a.mode === b.mode && a.screen === b.screen;

/** Where an import opens when its address names no screen: the files a draft has, or the run of one that started. */
export const firstPlace = (detail: ImportDetail): Place => {
  if (detail.status !== 'draft') {
    return { mode: 'simple', screen: 'add' };
  }
  return { mode: 'simple', screen: detail.plan.sources.length === 0 ? 'files' : 'found' };
};

export const importHref = (id: ImportId): string => `#/import/${String(id)}`;

export const placeHref = (id: ImportId, place: Place): string => `${importHref(id)}/${place.mode}/${place.screen}`;

const simpleScreen = (slug: string): SimpleScreen | undefined => SIMPLE_SCREENS.find((id) => id === slug);
const advancedScreen = (slug: string): AdvancedScreen | undefined => ADVANCED_SCREENS.find((id) => id === slug);

/** The place `[mode, screen]` names, `null` when the address names none, or `'unknown'` for one no screen has. */
export const placeFromSegments = (segments: readonly string[]): Place | null | 'unknown' => {
  const [mode, screen] = segments;
  if (mode === undefined) {
    return null;
  }
  if (mode === 'simple') {
    const id = simpleScreen(screen ?? 'files');
    return id === undefined ? 'unknown' : { mode, screen: id };
  }
  if (mode === 'advanced') {
    const id = advancedScreen(screen ?? 'files');
    return id === undefined ? 'unknown' : { mode, screen: id };
  }
  return 'unknown';
};
