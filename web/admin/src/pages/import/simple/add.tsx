/**
 * "Add them": what the run writes, from the server's check, as label and
 * value rows (the action, items, files, what is left out, the new item sets,
 * item types and fields, items with no item type, titles from the item code,
 * lists whose rows describe files) and who can see them. A delete writes
 * nothing and cannot be undone, so it has neither the visibility nor the undo
 * note; its start asks for the count typed, as in Advanced. The start, its
 * refusals, the progress and the end are the run screen's (`run/`), the one
 * start path both modes share; this file gives it Simple's rows for the check.
 */
import { Fragment, useEffect, useRef, useState } from 'react';
import type { ReactElement, ReactNode } from 'react';
import type { Action } from '../../../api/import-plan';
import type { Check } from '../../../api/import-sections';
import { useDraft } from '../draft-context';
import RunScreen from '../run/run-screen';
import { usePreview } from '../lists/use-preview';
import { placeHref } from '../screens';
import { checkFactsOf, titledByCode, withSuggestions } from './causes';
import { UNTITLED } from './questions';
import type { CauseId } from './questions';
import { readState, withState } from './example-state';
import { useSimpleText } from './messages';
import type { SimpleMessageKey } from './messages';
import { allLists } from '../series';
import './simple.css';

const ACTION_WORDS: Readonly<Record<Action, SimpleMessageKey>> = {
  create: 'guided.add.create',
  append: 'guided.add.append',
  revise: 'guided.add.revise',
  update: 'guided.add.update',
  replace: 'guided.add.replace',
  delete: 'guided.add.delete',
};

const NAMES_SHOWN = 10;

const counted = (check: Check, kind: 'item' | 'media'): number =>
  check.counts.filter((count) => count.kind === kind && count.does !== 'leave').reduce((sum, count) => sum + count.count, 0);

const left = (check: Check): number => check.counts.filter((count) => count.kind === 'item' && count.does === 'leave').reduce((sum, count) => sum + count.count, 0);

/** One row of the summary; a row with nothing to say is absent. */
interface Fact {
  readonly label: SimpleMessageKey;
  readonly value: ReactNode;
}

const Names = ({ names }: { readonly names: readonly string[] }): ReactElement => {
  const { t, number } = useSimpleText();
  const more = names.length - NAMES_SHOWN;
  return (
    <>
      {names.slice(0, NAMES_SHOWN).map((name) => (
        <span key={name} className="guided-chip">
          {name}
        </span>
      ))}
      {more > 0 ? <span className="muted">{t('guided.add.andMore', { count: number(more) })}</span> : null}
    </>
  );
};

/** The check in Simple's rows, above the run screen's start. */
export const SimpleSummary = ({ check }: { readonly check: Check }): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const out = left(check);
  const back = <a href={placeHref(draft.id, { mode: 'simple', screen: 'check' })}>{t('guided.add.titleChange')}</a>;
  const advanced = placeHref(draft.id, { mode: 'advanced', screen: 'existing' });
  const fileRows = allLists(draft.plan).filter((sheet) => sheet.rowKind === 'media');
  const facts: readonly (Fact | null)[] = [
    {
      label: 'guided.add.does',
      value: (
        <>
          {t(ACTION_WORDS[check.action])}
          {check.action === 'create' ? (
            <a className="muted" href={advanced}>
              {t('guided.add.inPlace')}
            </a>
          ) : (
            <span className="muted">
              {t('guided.add.chosenAdvanced')} <a href={advanced}>{t('guided.add.changeThere')}</a>
            </span>
          )}
        </>
      ),
    },
    { label: 'guided.add.items', value: <b>{number(counted(check, 'item'))}</b> },
    { label: 'guided.add.files', value: number(counted(check, 'media')) },
    out === 0 ? null : { label: 'guided.add.left', value: number(out) },
    check.newSets.length === 0 ? null : { label: 'guided.add.newSets', value: <Names names={check.newSets.map((set) => set.title)} /> },
    check.newTypes.length === 0 ? null : { label: 'guided.add.newTypes', value: <Names names={check.newTypes.map((type) => type.label)} /> },
    check.newFields.length === 0 ? null : { label: 'guided.add.newFields', value: <Names names={check.newFields.map((field) => field.label)} /> },
    check.withoutTemplate === 0 ? null : { label: 'guided.add.noType', value: number(check.withoutTemplate) },
    titledByCode(draft.plan)
      ? {
          label: 'guided.add.titles',
          value: (
            <>
              {t('guided.add.titleCode')} {back}
            </>
          ),
        }
      : null,
    fileRows.length === 0
      ? null
      : {
          label: 'guided.add.fileRows',
          value: (
            <>
              <Names names={fileRows.map((sheet) => sheet.file)} /> {back}
            </>
          ),
        },
  ];
  return (
    <div className="guided-dials">
      <dl className="guided-kv">
        {facts.map((fact) =>
          fact === null ? null : (
            <Fragment key={fact.label}>
              <dt>{t(fact.label)}</dt>
              <dd>{fact.value}</dd>
            </Fragment>
          ),
        )}
        {check.action === 'delete' ? null : (
          <>
            <dt>{t('guided.add.who')}</dt>
            <dd data-decision="who">
              <select
                aria-label={t('guided.add.who')}
                value={draft.plan.defaults.visibility}
                disabled={!draft.editable}
                onChange={(event) => {
                  const visibility = event.target.value === 'public' ? 'public' : 'private';
                  draft.change((plan) => ({ ...plan, defaults: { ...plan.defaults, visibility } }));
                }}
              >
                <option value="private">{t('guided.add.private')}</option>
                <option value="public">{t('guided.add.public')}</option>
              </select>
            </dd>
          </>
        )}
      </dl>
      {check.blocking.length === 0 ? null : (
        <p role="alert">
          {check.blocking.length === 1 ? t('guided.add.mustFixOne') : t('guided.add.mustFix', { count: number(check.blocking.length) })}{' '}
          <a href={placeHref(draft.id, { mode: 'simple', screen: 'check' })}>{t('guided.add.lookAt')}</a>
        </p>
      )}
      {check.action === 'delete' ? null : <p className="muted">{t('guided.add.undo')}</p>}
    </div>
  );
};

/**
 * Check promises that a skipped question gets its suggested answer, and some
 * suggestions are plan edits the run does not make on its own (`withSuggestions`).
 * So this screen writes them once for the questions nobody answered, and the
 * run screen waits for that, so its Start never counts a plan about to change.
 */
const AddThem = (): ReactElement => {
  const { t } = useSimpleText();
  const draft = useDraft();
  const read = usePreview(['flags', 'check', 'histogram', 'listCandidates'], {}, draft.editable);
  const tried = useRef(false);
  const [settled, setSettled] = useState(!draft.editable);
  useEffect(() => {
    if (tried.current || read.status === 'loading') {
      return;
    }
    tried.current = true;
    if (read.status === 'ready') {
      const { answered } = readState(draft.plan);
      const untitled: readonly CauseId[] = read.data.check.itemsWithoutTitle > 0 ? [UNTITLED] : [];
      const counted: readonly CauseId[] = [...read.data.flags.filter((flag) => flag.count > 0).map((flag) => flag.kind), ...untitled];
      const open = counted.filter((id) => !answered.includes(id));
      const facts = checkFactsOf(draft.plan, read.data);
      const suggested = withSuggestions(draft.plan, open, facts);
      if (suggested.applied.length > 0) {
        draft.change((plan) => withState(withSuggestions(plan, open, facts).plan, { answered: [...readState(plan).answered, ...suggested.applied] }));
      }
    }
    setSettled(true);
  }, [read, draft]);
  return settled ? <RunScreen summary={SimpleSummary} /> : <p role="status" className="import-slow">{t('shell.loading')}</p>;
};

export default AddThem;
