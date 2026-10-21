/**
 * "What we found": what the files make under the grouping shown on the same
 * screen, never a guess. Before an example, files are one item only when
 * their names are equal but for the ending, and the screen says so; the
 * person shows one example to say more. Then the files chart, the groups of
 * items that share facts, and one answer for every group.
 *
 * A draft starts with the pattern the screen states (the server's default
 * plan). A pattern cleared in Advanced makes no item at all; the screen says
 * so and offers that pattern back, and never writes it unasked.
 */
import { useEffect, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { ErrorNotice } from '../../../app/errors';
import { useDraft } from '../draft-context';
import { FilesChart } from '../lists/histogram';
import { Groups } from '../lists/groups';
import { listHref } from '../lists/address';
import { usePreview } from '../lists/use-preview';
import { placeHref } from '../screens';
import { patternOf } from '../example';
import { readState, typedElsewhere, withMarks, withState } from './example-state';
import { folderSheets, listEdit } from './list-read';
import { marksWords } from './mark-words';
import { useSimpleText } from './messages';
import './simple.css';

const Grouping = (): ReactElement => {
  const { t } = useSimpleText();
  const draft = useDraft();
  const { marks } = readState(draft.plan);
  const example = placeHref(draft.id, { mode: 'simple', screen: 'example' });
  if (typedElsewhere(draft.plan)) {
    return (
      <div className="guided-ask" data-decision="example">
        <p>{t('guided.found.typed')}</p>
        <p>
          <a href={placeHref(draft.id, { mode: 'advanced', screen: 'files' })}>{t('guided.found.seeAdvanced')}</a>
        </p>
      </div>
    );
  }
  if (marks.length === 0) {
    return (
      <div className="guided-ask" data-decision="example">
        <p className="guided-ask-q">{t('guided.found.ask')}</p>
        <p className="muted">{t('guided.found.now')}</p>
        <p>
          <a className="button-link import-primary" href={example}>
            {t('guided.found.show')}
          </a>
        </p>
      </div>
    );
  }
  return (
    <div className="guided-ask guided-done" data-decision="example">
      <p>{t('guided.found.marked', { marks: marksWords(marks, t) })}</p>
      <p className="import-row-actions">
        <a className="button-link" href={example}>
          {t('guided.found.another')}
        </a>
        <button
          type="button"
          disabled={!draft.editable}
          onClick={() => {
            draft.change((plan) => withMarks(plan, [], null));
          }}
        >
          {t('guided.found.forget')}
        </button>
      </p>
    </div>
  );
};

const AllRight = ({ groups }: { readonly groups: number }): ReactElement | null => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const pattern = draft.plan.grouping.pattern;
  if (groups === 0) {
    return null;
  }
  if (readState(draft.plan).groupsOk === pattern) {
    return <p className="ok-text">{t('guided.found.allOk')}</p>;
  }
  return (
    <div className="guided-ask">
      <p className="guided-ask-q">{groups === 1 ? t('guided.found.rightOne') : t('guided.found.right', { count: number(groups) })}</p>
      <p>
        <button
          type="button"
          className="import-primary"
          disabled={!draft.editable}
          onClick={() => {
            draft.change((plan) => withState(plan, { groupsOk: plan.grouping.pattern }));
          }}
        >
          {t('guided.found.useSuggested')}
        </button>
      </p>
      <p className="muted">{t('guided.found.later')}</p>
    </div>
  );
};

export const WhatWeFound = (): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const unset = draft.plan.grouping.pattern === null;
  const read = usePreview(['shapes', 'flags'], { shapes: { page: 1, perPage: 1 } });
  const listing = useRef(false);
  const [listError, setListError] = useState<Error | null>(null);
  // Only before the person answered about any list, and once the pattern is set: the pairing counts depend on it.
  const untouched = !unset && folderSheets(draft.plan).length === 0 && readState(draft.plan).notLists.length === 0;
  const candidates = usePreview(['listCandidates'], {}, untouched);
  const offer = candidates.status === 'ready' && untouched ? candidates.data.listCandidates.find((candidate) => (candidate.best?.paired ?? 0) > 0) : undefined;
  useEffect(() => {
    if (offer === undefined || listing.current || !draft.editable) {
      return;
    }
    listing.current = true;
    listEdit(draft.id, draft.plan, offer).then(
      (edit) => {
        draft.change(edit);
      },
      (error: unknown) => {
        setListError(error instanceof Error ? error : new Error(String(error)));
      },
    );
  }, [offer, draft]);
  if (unset) {
    return (
      <div className="import-note">
        <p>{t('guided.found.ungrouped')}</p>
        <p>
          <button
            type="button"
            className="import-primary"
            disabled={!draft.editable}
            onClick={() => {
              draft.change((plan) => ({ ...plan, grouping: { ...plan.grouping, pattern: patternOf([]) } }));
            }}
          >
            {t('guided.found.byName')}
          </button>
        </p>
      </div>
    );
  }
  if (read.status === 'loading') {
    return <p role="status" className="import-slow">{t('shell.loading')}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const { shapes, flags, sources } = read.data;
  const files = sources.reduce((sum, source) => sum + source.files, 0);
  const notMedia = flags.filter((flag) => flag.kind === 'not-media' && flag.policy === 'leave').reduce((sum, flag) => sum + flag.count, 0);
  const allFiles = { mode: 'simple', screen: 'all-files' } as const;
  return (
    <>
      <p className="guided-big">
        {t('guided.found.lede', { files: number(files) })}{' '}
        <b>{shapes.records === 1 ? t('guided.found.itemsOne') : t('guided.found.items', { count: number(shapes.records) })}</b>{' '}
        <span className="muted">{t('guided.found.gloss')}</span>
      </p>
      {listError === null ? null : <ErrorNotice error={listError} />}
      <Grouping />
      <FilesChart />
      <Groups />
      <AllRight groups={shapes.groups.total} />
      {notMedia === 0 ? null : (
        <p className="muted">
          {notMedia === 1 ? t('guided.found.notMediaOne') : t('guided.found.notMedia', { count: number(notMedia) })}{' '}
          <a href={listHref(draft.id, allFiles, { reason: 'flag' })}>{t('guided.found.showThem')}</a>
        </p>
      )}
      <p>
        <a href={placeHref(draft.id, allFiles)}>{t('guided.found.everyFile')}</a>
      </p>
    </>
  );
};

export default WhatWeFound;
