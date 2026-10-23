/**
 * The parts every node of the Structure screen is drawn with: the screen's
 * context (the clicks, the proposal on screen, the words), a question's
 * segmented answers, an offer's button, the inspector's rows, and whether
 * a folder is imported. The screen and its lazy chunks
 * (`structure-nodes.tsx`, `structure-hand.tsx`, `structure-changes.tsx`)
 * share them.
 */
import { createContext, useContext, useLayoutEffect, useRef } from 'react';
import type { ReactElement, ReactNode } from 'react';
import type { Proposal, ScopeCounts } from '../../../api/import-proposal';
import type { Edit } from '../../../api/import-plan';
import type { Focus } from '../screens';
import { Segmented } from './master-detail';
import { QUESTIONS, heldBy } from './structure-edits';
import type { Clicks, Question as Asked, Write } from './structure-edits';
import { itemsWords, kindsWords } from './structure-lines';
import type { StructureTranslator } from './structure-lines';

export type Words = StructureTranslator;

/** One answer of a question: what it shows, and what a click on it sends. */
export interface Answer {
  /** `chosen`, `rival` or `none`: its place among the answers. */
  readonly value: string;
  readonly label: string;
  /** What this answer makes, shown before the click. */
  readonly note?: string;
  /** The suggestion's own answer: it carries the ✓. */
  readonly draft: boolean;
  /** The answer the proposal holds now. */
  readonly pressed: boolean;
  /** The edit a click sends; `null` takes the question's edit back. */
  readonly edit: Edit | null;
  /** The question the click answers, when it is not the control's own: a set's title is asked apart from whether it is made. */
  readonly asked?: Asked;
  /** What a click also writes into the plan, for a question no edit answers. */
  readonly write?: Write;
}

/** What every question reads: the clicks, whether the plan takes edits, the proposal and the words. */
export interface Screen {
  readonly clicks: Clicks;
  readonly editable: boolean;
  /** The proposal on screen: the one a click brought back, or the read. */
  readonly proposal: Proposal;
  readonly words: Words;
  /** Whether the question `kind` takes the focus: true once, for the question an old step's address named. */
  readonly takesFocus: (kind: Focus) => boolean;
}

export const ScreenContext = createContext<Screen | null>(null);

export const useScreen = (): Screen => {
  const screen = useContext(ScreenContext);
  if (screen === null) {
    throw new Error('a question is drawn outside the Structure screen');
  }
  return screen;
};

/** A question's control: the answer the proposal holds is pressed, and a click on another sends its edit. */
export const Question = ({ label, asked, answers, kind }: { readonly label: string; readonly asked: Asked; readonly answers: readonly Answer[]; readonly kind?: Focus }): ReactElement => {
  const { clicks, editable, words, takesFocus } = useScreen();
  const asks = new Set([asked, ...answers.flatMap((answer) => (answer.asked === undefined ? [] : [answer.asked]))]);
  const waiting = clicks.pending !== null && asks.has(clicks.pending.asked) ? clicks.pending : null;
  const pressed = waiting?.value ?? answers.find((answer) => answer.pressed)?.value ?? null;
  const here = useRef<HTMLDivElement>(null);
  // In the commit that draws the question, so the focus is there as soon as it shows; only when it first shows.
  useLayoutEffect(() => {
    if (kind !== undefined && takesFocus(kind)) {
      here.current?.querySelector<HTMLButtonElement>('[aria-pressed="true"]')?.focus();
    }
  }, []);
  return (
    <div ref={here} className="structure-question">
      <Segmented
        label={label}
        segments={answers.map((answer) => ({
          value: answer.value,
          label: answer.label,
          suggested: answer.draft,
          ...(answer.note === undefined ? {} : { note: answer.note }),
        }))}
        value={pressed}
        disabled={!editable}
        suggestedWord={words.t('structure.suggested')}
        onChange={(value) => {
          const answer = answers.find((one) => one.value === value);
          if (answer !== undefined) {
            clicks.answer(answer.asked ?? asked, value, answer.edit, answer.write);
          }
        }}
      />
      {waiting?.slow === true ? (
        <p role="status" className="import-slow">
          {words.t('structure.counting')}
        </p>
      ) : null}
      {clicks.failed !== null && asks.has(clicks.failed) ? <p role="alert">{words.t('structure.click.failed')}</p> : null}
    </div>
  );
};

export const Row = ({ label, children }: { readonly label: string; readonly children: ReactNode }): ReactElement => (
  <>
    <dt>{label}</dt>
    <dd>{children}</dd>
  </>
);

export const Notes = ({ lines }: { readonly lines: readonly string[] }): ReactElement | null =>
  lines.length === 0 ? null : (
    <div className="structure-notes">
      {lines.map((line) => (
        <p key={line}>{line}</p>
      ))}
    </div>
  );

/** An offer: one button that sends its edit. */
export const Offer = ({ text, action, asked, edit }: { readonly text: ReactNode; readonly action: string; readonly asked: Asked; readonly edit: Edit }): ReactElement => {
  const { clicks, editable, words } = useScreen();
  return (
    <div className="structure-offer">
      <span>{text}</span>
      <button
        type="button"
        disabled={!editable}
        onClick={() => {
          clicks.answer(asked, 'offer', edit);
        }}
      >
        {action}
      </button>
      {clicks.failed === asked ? <p role="alert">{words.t('structure.click.failed')}</p> : null}
    </div>
  );
};

/** Import this folder, or leave it out: offered on a folder written by hand too. */
export const ImportQuestion = ({ scope, items }: { readonly scope: ScopeCounts; readonly items: number }): ReactElement => {
  const { proposal, words } = useScreen();
  const { t, number } = words;
  const asked = QUESTIONS.import(scope.scope);
  const out = heldBy(proposal, asked)?.applied === true;
  return (
    <Row label={t('structure.q.import')}>
      <Question
        label={t('structure.q.import')}
        asked={asked}
        answers={[
          {
            value: 'chosen',
            label: t('structure.import.yes'),
            draft: !out,
            pressed: true,
            note: t('structure.import.files', { files: number(scope.files), kinds: kindsWords(scope.kinds, t, number) }),
            edit: null,
          },
          {
            value: 'rival',
            label: t('structure.import.no'),
            draft: out,
            pressed: false,
            note: t('structure.import.noCount', { items: itemsWords(items, t, number) }),
            edit: out ? null : { edit: 'leaveOut', scope: scope.scope },
          },
        ]}
      />
    </Row>
  );
};
