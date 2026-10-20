/**
 * "Add them": what the run writes, in plain words, from the server's check
 * (items and files, the new item sets, item types and fields, items with no
 * item type), who can see them, and the start. The start sends the digest of
 * the check on screen, so a change since then answers "changed" and nothing
 * runs that the person did not see. What the run does next, its progress and
 * its end, is the run screens' (`run/`).
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import type { Action } from '../../../api/import-plan';
import { importApi } from '../../../api/import-routes';
import type { Check } from '../../../api/import-sections';
import { ErrorNotice } from '../../../app/errors';
import { toError } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { usePreview } from '../lists/use-preview';
import { placeHref } from '../screens';
import { useSimpleText } from './messages';
import type { SimpleMessageKey } from './messages';
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

const Names = ({ names, one, many }: { readonly names: readonly string[]; readonly one: SimpleMessageKey; readonly many: SimpleMessageKey }): ReactElement | null => {
  const { t, number } = useSimpleText();
  if (names.length === 0) {
    return null;
  }
  const shown = names.slice(0, NAMES_SHOWN).map((name) => `“${name}”`).join(', ');
  const more = names.length - NAMES_SHOWN;
  return (
    <li>
      {names.length === 1 ? t(one, { names: shown }) : t(many, { count: number(names.length), names: shown })}
      {more > 0 ? ` ${t('guided.add.andMore', { count: number(more) })}` : null}
    </li>
  );
};

type Start = { readonly at: 'idle' } | { readonly at: 'starting' } | { readonly at: 'started' } | { readonly at: 'failed'; readonly error: Error };

export const AddThem = (): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const [start, setStart] = useState<Start>({ at: 'idle' });
  const read = usePreview(['check'], {});
  if (draft.detail.status !== 'draft') {
    return <p role="status">{start.at === 'started' ? t('guided.add.going') : t('guided.add.started')}</p>;
  }
  if (read.status === 'loading') {
    return <p role="status" className="import-slow">{t('shell.loading')}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const { check } = read.data;
  const items = counted(check, 'item');
  const files = counted(check, 'media');
  const out = left(check);
  const saved = draft.save.status === 'saved';
  const run = (): void => {
    setStart({ at: 'starting' });
    importApi.run(draft.id, { digest: check.digest }).then(
      () =>
        importApi.get(draft.id).then((detail) => {
          draft.adopt(detail);
          setStart({ at: 'started' });
        }),
      (error: unknown) => {
        setStart({ at: 'failed', error: toError(error) });
      },
    ).catch((error: unknown) => {
      setStart({ at: 'failed', error: toError(error) });
    });
  };
  return (
    <>
      <p className="guided-big" aria-busy={read.busy}>
        {t('guided.add.ready', { what: t(ACTION_WORDS[check.action]) })}{' '}
        <b>{items === 1 ? t('guided.add.itemsOne') : t('guided.add.items', { count: number(items) })}</b>
        {', '}
        {files === 1 ? t('guided.add.filesOne') : t('guided.add.files', { count: number(files) })}.
      </p>
      {check.action === 'create' ? (
        <p className="muted">{t('guided.add.inPlace')}</p>
      ) : (
        <p className="import-note">
          {t('guided.add.chosenAdvanced')} <a href={placeHref(draft.id, { mode: 'advanced', screen: 'existing' })}>{t('guided.add.changeThere')}</a>
        </p>
      )}
      <ul>
        {out === 0 ? null : <li>{out === 1 ? t('guided.add.leftOne') : t('guided.add.left', { count: number(out) })}</li>}
        <Names names={check.newSets.map((set) => set.title)} one="guided.add.newSetOne" many="guided.add.newSets" />
        <Names names={check.newTypes.map((type) => type.label)} one="guided.add.newTypeOne" many="guided.add.newTypes" />
        <Names names={check.newFields.map((field) => field.label)} one="guided.add.newFieldOne" many="guided.add.newFields" />
        {check.withoutTemplate === 0 ? null : (
          <li>{check.withoutTemplate === 1 ? t('guided.add.noTypeOne') : t('guided.add.noType', { count: number(check.withoutTemplate) })}</li>
        )}
      </ul>
      {check.blocking.length === 0 ? null : (
        <p role="alert">
          {check.blocking.length === 1 ? t('guided.add.mustFixOne') : t('guided.add.mustFix', { count: number(check.blocking.length) })}{' '}
          <a href={placeHref(draft.id, { mode: 'simple', screen: 'check' })}>{t('guided.add.lookAt')}</a>
        </p>
      )}
      <p>
        <label data-decision="who">
          {t('guided.add.who')}{' '}
          <select
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
        </label>
      </p>
      <p className="muted">{t('guided.add.undo')}</p>
      {check.action === 'delete' ? (
        <p className="import-note">{t('guided.add.deleteAdvanced')}</p>
      ) : (
        <p>
          <button type="button" className="import-primary guided-big" disabled={!check.canRun || !saved || !draft.editable || start.at === 'starting'} onClick={run}>
            {start.at === 'starting' ? t('guided.add.starting') : items === 1 ? t('guided.add.startOne') : t('guided.add.start', { count: number(items) })}
          </button>
        </p>
      )}
      {saved ? null : <p className="muted">{t('guided.add.waitSave')}</p>}
      {start.at === 'failed' ? <ErrorNotice error={start.error} /> : null}
      {start.at === 'started' ? <p role="status">{t('guided.add.going')}</p> : null}
    </>
  );
};

export default AddThem;
