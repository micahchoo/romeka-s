/**
 * The screens of one import, in both modes, and the addresses that name them:
 * `#/import/<id>/<mode>/<screen>`. Simple and Advanced are two views of one
 * plan, so every screen names its counterpart in the other mode, and the mode
 * switch lands there (`docs/import-v2.md`, "Simple and Advanced").
 *
 * A screen without a `step` is a sub-screen: the step bar or the side index
 * marks the nearest `parent` with a step as the place the person is.
 *
 * Simple's steps are Structure, Check and Add. "What we found", "Your list"
 * and "Where they go" were steps; each became questions of a Structure node.
 * They stay as sub-screens of Structure for a plan of one series, which they
 * edit; on a plan of several their addresses, and the marked example's, land
 * on Structure, at the question they asked (`landing`).
 *
 * Advanced's Structure is the same tree, each node with its rules (the rule
 * zoom). Files, Lists and Fields were Advanced screens; their contents are
 * sections of a node now, so their addresses open Structure at that section.
 */
import type { ImportDetail } from '../../api/import-view';
import type { ImportId, ImportPlan } from '../../api/import-plan';
import type { ImportMessageKey } from './messages';
import { onlySeries } from './series';

export const SIMPLE_SCREENS = [
  'structure',
  'files',
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
  'structure',
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

/** The question of a Structure node an old step's address lands on. */
export type Focus = 'items' | 'list' | 'set';

/** A section of a node in Advanced's rule zoom. */
export type Section = 'files' | 'items' | 'lists' | 'fields' | 'set';

export type Place =
  | { readonly mode: 'simple'; readonly screen: SimpleScreen; readonly focus?: Focus }
  | { readonly mode: 'advanced'; readonly screen: AdvancedScreen; readonly section?: Section };

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
  structure: { title: 'simple.structure.title', step: 'simple.structure.step', other: 'structure' },
  files: { title: 'simple.files.title', parent: 'structure', other: 'structure' },
  found: { title: 'simple.found.title', parent: 'structure', other: 'structure' },
  example: { title: 'simple.example.title', parent: 'found', other: 'structure' },
  'all-files': { title: 'simple.allFiles.title', parent: 'found', other: 'all-files' },
  list: { title: 'simple.list.title', parent: 'structure', other: 'structure' },
  pair: { title: 'simple.pair.title', parent: 'list', other: 'pair' },
  where: { title: 'simple.where.title', parent: 'structure', other: 'structure' },
  check: { title: 'simple.check.title', step: 'simple.check.step', other: 'problems' },
  'all-items': { title: 'simple.allItems.title', parent: 'check', other: 'items' },
  add: { title: 'simple.add.title', step: 'simple.add.step', other: 'run' },
};

export const ADVANCED: Readonly<Record<AdvancedScreen, Screen<AdvancedScreen, SimpleScreen>>> = {
  structure: { title: 'advanced.structure.title', step: 'advanced.structure.step', other: 'structure' },
  problems: { title: 'advanced.problems.title', step: 'advanced.problems.step', other: 'check' },
  existing: { title: 'advanced.existing.title', step: 'advanced.existing.step', other: 'add' },
  changes: { title: 'advanced.changes.title', step: 'advanced.changes.step', other: 'check' },
  items: { title: 'advanced.items.title', step: 'advanced.items.step', other: 'all-items' },
  run: { title: 'advanced.run.title', step: 'advanced.run.step', other: 'add' },
  'all-files': { title: 'advanced.allFiles.title', parent: 'structure', other: 'all-files' },
  pair: { title: 'advanced.pair.title', parent: 'structure', other: 'pair' },
};

/** The screen itself when it is a step, else its nearest parent that is one. */
const stepOf = <S extends string>(screens: Readonly<Record<S, Screen<S, string>>>, screen: S): S => {
  const { step, parent } = screens[screen];
  return step !== undefined || parent === undefined ? screen : stepOf(screens, parent);
};

/** The step bar's entries (Simple) or the side index's (Advanced), in order, and which one is current. */
export const stepsOf = (place: Place): { readonly steps: readonly Place[]; readonly current: Place } => {
  if (place.mode === 'simple') {
    return {
      steps: SIMPLE_SCREENS.filter((id) => SIMPLE[id].step !== undefined).map((id) => ({ mode: 'simple', screen: id })),
      current: { mode: 'simple', screen: stepOf(SIMPLE, place.screen) },
    };
  }
  return {
    steps: ADVANCED_SCREENS.filter((id) => ADVANCED[id].step !== undefined).map((id) => ({ mode: 'advanced', screen: id })),
    current: { mode: 'advanced', screen: stepOf(ADVANCED, place.screen) },
  };
};

/** The screen a sub-screen opens from; a step has none. */
export const parentOf = (place: Place): Place | undefined => {
  if (place.mode === 'simple') {
    const parent = SIMPLE[place.screen].parent;
    return parent === undefined ? undefined : { mode: 'simple', screen: parent };
  }
  const parent = ADVANCED[place.screen].parent;
  return parent === undefined ? undefined : { mode: 'advanced', screen: parent };
};

export const screenOf = (place: Place): Screen<string, string> =>
  place.mode === 'simple' ? SIMPLE[place.screen] : ADVANCED[place.screen];

export const counterpart = (place: Place): Place =>
  place.mode === 'simple'
    ? { mode: 'advanced', screen: SIMPLE[place.screen].other }
    : { mode: 'simple', screen: ADVANCED[place.screen].other };

export const samePlace = (a: Place, b: Place): boolean => a.mode === b.mode && a.screen === b.screen;

/** Where an import opens when its address names no screen: a draft's structure (its files first, when it has none), or the run of one that started. */
export const firstPlace = (detail: ImportDetail): Place => {
  if (detail.status !== 'draft') {
    return { mode: 'simple', screen: 'add' };
  }
  return { mode: 'simple', screen: detail.plan.sources.length === 0 ? 'files' : 'structure' };
};

/** The old steps and the example they open, and the Structure question each one asked. */
const MOVED: Readonly<Partial<Record<SimpleScreen, Focus>>> = { found: 'items', example: 'items', list: 'list', where: 'set' };

/**
 * The place a screen opens as, for this plan: an old step of Simple edits the
 * plan's one series, so on a plan of several it lands on Structure at the
 * question it asked. Every other place opens as itself.
 */
export const landing = (place: Place, plan: ImportPlan): Place => {
  const focus = place.mode === 'simple' ? MOVED[place.screen] : undefined;
  return focus === undefined || onlySeries(plan) !== null ? place : { mode: 'simple', screen: 'structure', focus };
};

export const importHref = (id: ImportId): string => `#/import/${String(id)}`;

export const placeHref = (id: ImportId, place: Place): string => `${importHref(id)}/${place.mode}/${place.screen}`;

const simpleScreen = (slug: string): SimpleScreen | undefined => SIMPLE_SCREENS.find((id) => id === slug);
const advancedScreen = (slug: string): AdvancedScreen | undefined => ADVANCED_SCREENS.find((id) => id === slug);

/** Advanced's old screens, by the section of a node each one's contents moved to. */
const MOVED_ADVANCED = new Map<string, Section>([
  ['files', 'items'],
  ['lists', 'lists'],
  ['fields', 'fields'],
]);

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
    const section = MOVED_ADVANCED.get(screen ?? '');
    const id = advancedScreen(screen ?? 'structure');
    return section !== undefined ? { mode, screen: 'structure', section } : id === undefined ? 'unknown' : { mode, screen: id };
  }
  return 'unknown';
};
