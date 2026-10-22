/**
 * The write rule of Advanced (P5 design, section 1). A click on the
 * Structure screen saves the proposal's structure, and the proposer copies
 * some members of a suggested folder from the plan and works the others out
 * again. So every Advanced write has a kind, and `WRITES` holds it:
 *
 * - `carried`: the proposer copies the member (a list's read and column
 *   options, slot rules, free sets, uploaded lists) or never reads it (the
 *   plan's defaults, flags, action, decisions). The write is the plan's.
 * - `owned`: the proposer works the member out again (the pattern, the
 *   marked example, the join, what a row describes, which lists a folder
 *   reads). On a suggested folder the same save adds one `own` edit, so the
 *   proposer keeps the folder as written and the screen says "written by you".
 * - `offered`: the proposal offers the choice, so Advanced sends the edit
 *   Simple sends, and the server replays it. No Advanced setting writes one
 *   yet: the rule zoom (unit C2) sends them through the Structure clicks.
 *
 * Every function of `plan-edits.ts` that writes the plan is a key of
 * `WRITES`, so the type checker refuses a writer with no kind.
 */
import type { Edit, FolderScope, ImportPlan, LevelId, SeriesScope } from '../../../api/import-plan';
import { isAsFound, seriesOf } from '../series';
import type * as PlanEdits from './plan-edits';

export type WriteKind = 'offered' | 'carried' | 'owned';

/** The functions of `module` that take a plan and give one back. */
type WritersOf<M> = keyof { [K in keyof M as M[K] extends (plan: ImportPlan, ...rest: never[]) => ImportPlan ? K : never]: K };

/** Writes made outside `plan-edits.ts`, by what they change. */
type OtherWrites =
  /** A list read into a folder, moved to it, or taken out (`lists/list-picker.tsx`). */
  | 'list'
  /** What each row of a list describes (`simple/list-plan.ts`, `withRowKind`). */
  | 'rowKind'
  /** A column on the item or on each file (`simple/list-plan.ts`, `withColumnLevel`). */
  | 'columnLevel'
  /** A mark of the example taken away, or the example (`example.ts`). */
  | 'example';

export type Writer = WritersOf<typeof PlanEdits> | OtherWrites;

export const WRITES = {
  setRead: 'carried',
  setJoinColumn: 'owned',
  setJoin: 'owned',
  mapColumn: 'carried',
  setRowKindColumn: 'owned',
  setItemKey: 'owned',
  setColumnOptions: 'carried',
  addRule: 'carried',
  removeRule: 'carried',
  moveSource: 'carried',
  removeSource: 'carried',
  addSource: 'carried',
  setCombine: 'carried',
  setMapped: 'carried',
  setDefaults: 'carried',
  setUnmatched: 'carried',
  setFlagPolicy: 'carried',
  setAnomalies: 'carried',
  setAction: 'carried',
  setMatch: 'carried',
  removeDecision: 'carried',
  removeDecisions: 'carried',
  setPattern: 'owned',
  list: 'owned',
  rowKind: 'owned',
  columnLevel: 'carried',
  example: 'owned',
} as const satisfies Record<Writer, WriteKind>;

/** Where a folder stands for the proposer: the archive as found (no proposal holds it yet), suggested, or written by the person. */
export type Standing = 'asFound' | 'suggested' | 'written';

const sameScope = (a: FolderScope, b: FolderScope): boolean => a.source === b.source && a.folder === b.folder;

/** Whether `edit` keeps the series of this scope as written: the proposer's own rule (`replay_own`). */
const owns = (edit: Edit, scope: SeriesScope): boolean =>
  edit.edit === 'own' && (scope === 'all' ? edit.scope.folder === '' : scope.some((one) => sameScope(one, edit.scope)));

export const standingOf = (plan: ImportPlan, level: LevelId): Standing => {
  if (isAsFound(plan)) {
    return 'asFound';
  }
  const { scope } = seriesOf(plan, level);
  return (plan.edits ?? []).some((edit) => owns(edit, scope)) ? 'written' : 'suggested';
};

/** The folder an `own` edit names for the series `level`: its first, or the whole first source. */
export const ownScopeOf = (plan: ImportPlan, level: LevelId): FolderScope | null => {
  const { scope } = seriesOf(plan, level);
  if (scope !== 'all') {
    return scope[0] ?? null;
  }
  const [first] = plan.sources;
  return first === undefined ? null : { source: first.id, folder: '' };
};

/**
 * The plan after `change`, the write `name` on the series `level` (none for
 * a write of the whole plan). An owned write that changes a suggested folder
 * also adds the `own` edit of that folder, last, as the person's newest edit.
 */
export const written = (plan: ImportPlan, name: Writer, level: LevelId | undefined, change: (plan: ImportPlan) => ImportPlan): ImportPlan => {
  const next = change(plan);
  if (next === plan || WRITES[name] !== 'owned' || level === undefined || standingOf(plan, level) !== 'suggested') {
    return next;
  }
  const scope = ownScopeOf(plan, level);
  return scope === null ? next : { ...next, edits: [...(next.edits ?? []), { edit: 'own', scope }] };
};
