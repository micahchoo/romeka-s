/**
 * The clicks of the Structure screen. A click answers one question of one
 * node, and a question holds at most one edit of `plan.edits`: a new answer
 * takes the place of the question's edit, and a click back on the
 * suggestion's answer deletes it. So the person's changes never grow from
 * going back and forth.
 *
 * A click is tried, then saved: the admin asks for the proposal with the new
 * edits, and when it comes back saves the edits and the proposal's
 * structure in one save. The first click therefore also takes the whole
 * suggested structure (design call 1). A count that is slow to come back
 * does not hold the edit: after `SAVE_FIRST_MS` the edits are saved alone.
 *
 * A hand edit also writes the plan: it changes one series of the shown
 * structure (`write`) and answers that folder's `own` question, so the
 * proposer keeps the series as written. The saved structure is then the
 * proposal's, which holds the written series verbatim.
 */
import { useEffect, useRef, useState } from 'react';
import { importApi } from '../../../api/import-routes';
import type { EditResult, ItemCut, Proposal } from '../../../api/import-proposal';
import type { CutEdit, Edit, FolderScope, ImportId, ImportPlan, ListFile, SetFrom } from '../../../api/import-plan';
import { useDraft } from '../draft-context';
import { withStructure } from '../series';
import { totalsOf } from './structure-lines';
import type { TreeNode } from './structure-lines';

/** The proposal's parser, which the screen loads as a chunk of its own. */
export type Parse = (raw: unknown) => Proposal;

/** After this long, the screen says it is counting. */
export const SLOW_MS = 1000;
/** After this long, the edits are saved before their counts come back. */
export const SAVE_FIRST_MS = 10_000;

/** JSON with sorted keys: the same value whatever order its members were written in. */
const canonical = (value: unknown): string =>
  JSON.stringify(value, (_key, member: unknown) =>
    typeof member === 'object' && member !== null && !Array.isArray(member)
      ? Object.fromEntries(Object.entries(member).sort(([a], [b]) => a.localeCompare(b)))
      : member,
  );

export const sameEdit = (a: Edit, b: Edit): boolean => canonical(a) === canonical(b);

const sameScope = (a: FolderScope, b: FolderScope): boolean => a.source === b.source && a.folder === b.folder;
const sameList = (a: ListFile | null, b: ListFile | null): boolean => a !== null && b !== null && a.source === b.source && a.file === b.file;

/** A question of the screen, by what it names in the archive: `QUESTIONS` makes each. */
export type Question = string;

const question = (kind: string, named: unknown): Question => `${kind}:${canonical(named)}`;

export const QUESTIONS = {
  import: (scope: FolderScope) => question('import', scope),
  items: (scope: FolderScope) => question('items', scope),
  list: (scope: FolderScope) => question('list', scope),
  grain: (list: ListFile) => question('grain', list),
  pair: (list: ListFile) => question('pair', list),
  /** Whether there is a set from `from`: an `addSet` or a `dropSet`. */
  set: (from: SetFrom) => question('set', from),
  /** What the set from `from` is: a `title` (with its description) or a set already in Omeka (`existing`). The server asks it apart from `set`. */
  title: (from: SetFrom) => question('title', from),
  /** Whether the folder's series is written by hand. */
  own: (scope: FolderScope) => question('own', scope),
  /** Where a column of a list whose rows describe files goes. No edit answers it: its click writes the column's rules. */
  column: (list: ListFile, index: number) => question('column', { list, index }),
};

/** The question an edit answers. */
export const questionOf = (edit: Edit): Question => {
  switch (edit.edit) {
    case 'leaveOut':
    case 'keep':
      return QUESTIONS.import(edit.scope);
    case 'items':
      return QUESTIONS.items(edit.scope);
    case 'list':
      return QUESTIONS.list(edit.scope);
    case 'grain':
      return QUESTIONS.grain(edit.list);
    case 'pair':
      return QUESTIONS.pair(edit.list);
    case 'addSet':
    case 'dropSet':
      return QUESTIONS.set(edit.from);
    case 'title':
    case 'existing':
      return QUESTIONS.title(edit.from);
    case 'own':
      return QUESTIONS.own(edit.scope);
  }
};

/**
 * The edits with `edit` as the one answer to `asked`, last; `null` deletes
 * the answer. A `dropSet` also takes back what the set was named: a set that
 * is not made has no title.
 */
export const withAnswer = (edits: readonly Edit[], asked: Question, edit: Edit | null): readonly Edit[] => {
  const untitled = edit?.edit === 'dropSet' ? QUESTIONS.title(edit.from) : null;
  return [...edits.filter((one) => questionOf(one) !== asked && questionOf(one) !== untitled), ...(edit === null ? [] : [edit])];
};

/** The edit that answers `asked`, with whether the proposal applied it. */
export const heldBy = (proposal: Proposal, asked: Question): EditResult | undefined =>
  proposal.edits.find((result) => questionOf(result.edit) === asked);

/** The cut an `items` edit can ask for; a cut by list rows is never one. */
export const cutEditOf = (cut: ItemCut): CutEdit | null => {
  switch (cut.cut) {
    case 'whole':
      return { cut: 'whole' };
    case 'places':
      return { cut: 'places', places: cut.places };
    case 'folder':
      return { cut: 'folder', depth: cut.depth };
    case 'rows':
      return null;
  }
};

/** Whether an edit names this node: its folder, a list it uses, or a set of either. */
const names = (node: TreeNode, edit: Edit): boolean => {
  if (node.kind === 'outside') {
    return (edit.edit === 'leaveOut' || edit.edit === 'keep') && sameScope(edit.scope, node.folder.scope);
  }
  if (node.kind === 'every') {
    return edit.edit === 'addSet' && node.offers.some((offer) => canonical(offer.value) === canonical(edit.from));
  }
  if (node.kind === 'kept') {
    // The split that made a kept level was taken before its folder was written by hand.
    if (edit.edit !== 'addSet') {
      return false;
    }
    const { from } = edit;
    return from.from === 'column' ? node.lists.some((one) => sameList(from.list, one)) : sameScope(from.scope, node.scope);
  }
  if (node.kind !== 'series' && node.kind !== 'own') {
    return false;
  }
  const scope = node.node.scope.scope;
  const lists = node.kind === 'series' ? [node.node.list.chosen.value] : node.lists;
  switch (edit.edit) {
    case 'leaveOut':
    case 'keep':
    case 'items':
    case 'list':
    case 'own':
      return sameScope(edit.scope, scope);
    case 'grain':
    case 'pair':
      return lists.some((one) => sameList(edit.list, one));
    case 'addSet':
    case 'dropSet':
    case 'title':
    case 'existing': {
      const { from } = edit;
      return from.from === 'column' ? lists.some((one) => sameList(from.list, one)) : sameScope(from.scope, scope);
    }
  }
};

/** The edits of the proposal that name this node. */
export const editsOn = (node: TreeNode, proposal: Proposal): readonly EditResult[] =>
  proposal.edits.filter((result) => names(node, result.edit));

/** The tree's node an edit names, if it names one. */
export const nodeOf = (edit: Edit, tree: readonly TreeNode[]): TreeNode | undefined => tree.find((node) => names(node, edit));

/** What a click changed in the items to add: the counts before it and after it, as the person saw them. */
export interface Effect {
  readonly before: number;
  readonly after: number;
}

/**
 * Each click's effect, kept for this tab: the server sends nothing for it,
 * and it is the difference between the two proposals around the click.
 */
const effects = new Map<string, Effect>();

const effectKey = (id: ImportId, edit: Edit): string => `${String(id)}:${canonical(edit)}`;

export const effectOf = (id: ImportId, edit: Edit): Effect | undefined => effects.get(effectKey(id, edit));

/** The plan with these edits; with none, the plan has no `edits` member, as the server writes it. */
const withEdits = (plan: ImportPlan, edits: readonly Edit[]): ImportPlan => {
  const next: { -readonly [K in keyof ImportPlan]: ImportPlan[K] } = { ...plan, edits };
  if (edits.length === 0) {
    delete next.edits;
  }
  return next;
};

/** The click that waits for its counts. */
export interface Pending {
  readonly asked: Question;
  /** The answer that turned on at once. */
  readonly value: string;
  /** Over `SLOW_MS`: the screen says it is counting. */
  readonly slow: boolean;
  /** Over `SAVE_FIRST_MS`: the edits are saved and the counts still come. */
  readonly saved: boolean;
}

/** A hand edit's change to the plan, made on the shown structure before the plan is tried. */
export type Write = (plan: ImportPlan) => ImportPlan;

export interface Clicks {
  /** The proposal to show: the one a click brought back, until the read after its save replaces it. */
  readonly shown: Proposal;
  readonly pending: Pending | null;
  /** The question whose last click could not be counted. */
  readonly failed: Question | null;
  /** Answers `asked` with `edit` (`null`: back to the suggestion), shown at once as `value`; a hand edit also `write`s the plan. */
  readonly answer: (asked: Question, value: string, edit: Edit | null, write?: Write) => void;
  /** The plan `answer` would try: what a hand edit counts before it is used. */
  readonly tried: (asked: Question, edit: Edit | null, write?: Write) => ImportPlan;
  /** Takes the shown structure as it is, with no edit. */
  readonly use: () => void;
}

/**
 * The clicks over `read`, the proposal of the stored plan. `under` is the
 * read's answer; a proposal a click brought back is shown until the read
 * answers again with something else. `parse` reads each proposal a click brings.
 */
export const useClicks = (read: Proposal, under: unknown, parse: Parse): Clicks => {
  const draft = useDraft();
  const [trial, setTrial] = useState<{ readonly proposal: Proposal; readonly under: unknown } | null>(null);
  const [pending, setPending] = useState<Pending | null>(null);
  const [failed, setFailed] = useState<Question | null>(null);
  const ticket = useRef(0);
  const asking = useRef<readonly Edit[] | null>(null);
  const timers = useRef<ReturnType<typeof setTimeout>[]>([]);
  const stop = (): void => {
    timers.current.forEach(clearTimeout);
    timers.current = [];
  };
  useEffect(() => stop, []);

  const shown = trial !== null && trial.under === under ? trial.proposal : read;
  const written = (plan: ImportPlan, write: Write | undefined): ImportPlan => (write === undefined ? plan : write(withStructure(plan, shown.structure)));
  const editsAfter = (asked: Question, edit: Edit | null): readonly Edit[] => withAnswer(asking.current ?? draft.plan.edits ?? [], asked, edit);

  const answer = (asked: Question, value: string, edit: Edit | null, write?: Write): void => {
    const edits = editsAfter(asked, edit);
    const before = totalsOf(shown).items;
    const mine = ++ticket.current;
    asking.current = edits;
    stop();
    setFailed(null);
    setPending({ asked, value, slow: false, saved: false });
    timers.current = [
      setTimeout(() => {
        setPending((now) => (now === null ? now : { ...now, slow: true }));
      }, SLOW_MS),
      setTimeout(() => {
        draft.change((plan) => withEdits(written(plan, write), edits));
        setPending((now) => (now === null ? now : { ...now, saved: true }));
      }, SAVE_FIRST_MS),
    ];
    importApi.preview(draft.id, ['proposal'], { plan: withEdits(written(draft.plan, write), edits) }).then(
      (data) => {
        if (mine !== ticket.current) {
          return;
        }
        stop();
        asking.current = null;
        const proposal = parse(data.proposal);
        if (edit !== null) {
          effects.set(effectKey(draft.id, edit), { before, after: totalsOf(proposal).items });
        }
        // The save below makes this structure the plan's, so the plan now matches it.
        setTrial({ proposal: { ...proposal, matchesPlan: true }, under });
        setPending(null);
        // The proposal holds a series written by hand as it was written, so its structure carries the write.
        draft.change((plan) => withStructure(withEdits(plan, edits), proposal.structure));
      },
      () => {
        if (mine !== ticket.current) {
          return;
        }
        stop();
        asking.current = null;
        setPending(null);
        setFailed(asked);
      },
    );
  };

  const use = (): void => {
    draft.change((plan) => withStructure(plan, shown.structure));
  };

  const tried = (asked: Question, edit: Edit | null, write?: Write): ImportPlan => withEdits(written(draft.plan, write), editsAfter(asked, edit));

  return { shown, pending, failed, answer, tried, use };
};
