/**
 * "What we found": one line of counts and the rule that makes items, the
 * files chart, then the groups of items that share facts beside the
 * inspector of the one selected. A group is named by chips of its facts,
 * never by a guess; the inspector shows example items with their files and
 * takes one change for the whole group. Before an example, files are one item
 * only when their names are equal but for the ending; the person shows one
 * example to say more.
 *
 * A draft starts with the pattern the screen states (the server's default
 * plan). A pattern cleared in Advanced makes no item at all; the screen says
 * so and offers that pattern back, and never writes it unasked.
 */
import { useEffect, useRef, useState } from 'react';
import type { ReactElement } from 'react';
import { importApi } from '../../../api/import-routes';
import type { RecordView } from '../../../api/import-model';
import type { FlagCount, ShapeGroup } from '../../../api/import-sections';
import type { ImportPlan } from '../../../api/import-plan';
import { ErrorNotice } from '../../../app/errors';
import { Pager } from '../../../app/list';
import { useDraft } from '../draft-context';
import { FilesChart } from '../lists/histogram';
import { listHref } from '../lists/address';
import { ChangeMany } from '../lists/change-many';
import { BUCKET_WORDS, MEDIA_WORDS } from '../lists/facts-words';
import { fileEntries, gather, itemEntry } from '../lists/gather';
import { fileName, recordName } from '../lists/record-words';
import { shapeKey } from '../lists/shape-filter';
import { usePreview } from '../lists/use-preview';
import { placeHref } from '../screens';
import { patternOf } from '../example';
import { readState, typedElsewhere, withMarks, withState } from './example-state';
import { folderSheets, listEdit } from './list-read';
import { MasterDetail } from './master-detail';
import { marksWords } from './mark-words';
import { useSimpleText } from './messages';
import type { SimpleTranslator } from './messages';
import './simple.css';

const GROUPS_PER_PAGE = 20;
const FILES_SHOWN = 4;

interface Chip {
  readonly text: string;
  readonly kind: 'fact' | 'folder' | 'warn';
}

/** A group's facts as chips: files per item, kinds, the marked words or pages, where its row comes from, its folders, and "worth a look". */
const chipsOf = (group: ShapeGroup, plan: ImportPlan, t: SimpleTranslator['t']): readonly Chip[] => {
  const { facts } = group;
  const chips: Chip[] = [{ text: t(BUCKET_WORDS[facts.files]), kind: 'fact' }];
  if (facts.media.length > 0) {
    chips.push({ text: facts.media.map((kind) => t(MEDIA_WORDS[kind])).join(t('lists.facts.and')), kind: 'fact' });
  }
  if (facts.parts !== null && facts.parts.length > 0) {
    chips.push({ text: facts.parts.map((word) => `“${word}”`).join(', '), kind: 'fact' });
  }
  if (facts.pages !== null && facts.pages.length > 0) {
    chips.push({ text: t('lists.facts.pages', { count: String(facts.pages.length) }), kind: 'fact' });
  }
  if (facts.origin === 'own') {
    chips.push({ text: t('guided.found.chipOwn'), kind: 'fact' });
  }
  if (facts.origin === 'row') {
    chips.push({ text: t('guided.found.chipRowOnly'), kind: 'fact' });
  } else if (plan.sheets.length > 0) {
    chips.push({ text: facts.row ? t('guided.found.chipRow') : t('guided.found.chipNoRow'), kind: 'fact' });
  }
  const folders = facts.folders.filter((folder) => folder !== '');
  chips.push(...(folders.length === 0 ? [{ text: t('guided.found.chipTop'), kind: 'folder' as const }] : folders.map((folder) => ({ text: folder, kind: 'folder' as const }))));
  if (group.odd) {
    chips.push({ text: t('guided.found.chipOdd'), kind: 'warn' });
  }
  return chips;
};

/** The chips that name a group in the list, where "worth a look" stands apart. */
const named = (chips: readonly Chip[]): readonly Chip[] => chips.filter((chip) => chip.kind !== 'warn');

const CHIP_CLASS: Readonly<Record<Chip['kind'], string>> = { fact: 'guided-chip', folder: 'guided-chip guided-file', warn: 'guided-chip is-warn' };

const Chips = ({ chips }: { readonly chips: readonly Chip[] }): ReactElement => (
  <>
    {chips.map((chip, index) => (
      <span key={`${chip.kind}:${chip.text}`}>
        {index === 0 ? null : ' '}
        <span className={CHIP_CLASS[chip.kind]}>{chip.text}</span>
      </span>
    ))}
  </>
);

/** The rule that makes items now, in one line, with what changes it. */
const Rule = (): ReactElement => {
  const { t } = useSimpleText();
  const draft = useDraft();
  const { marks } = readState(draft.plan);
  const example = placeHref(draft.id, { mode: 'simple', screen: 'example' });
  if (typedElsewhere(draft.plan)) {
    return (
      <span className="guided-rule" data-decision="example">
        {t('guided.found.typed')} <a href={placeHref(draft.id, { mode: 'advanced', screen: 'files' })}>{t('guided.found.seeAdvanced')}</a>
      </span>
    );
  }
  if (marks.length === 0) {
    return (
      <span className="guided-rule" data-decision="example">
        {t('guided.found.byEnding')}{' '}
        <a className="button-link import-primary" href={example}>
          {t('guided.found.show')}
        </a>
      </span>
    );
  }
  return (
    <span className="guided-rule" data-decision="example">
      {t('guided.found.byMark', { marks: marksWords(marks, t) })}{' '}
      <a className="button-link" href={example}>
        {t('guided.found.another')}
      </a>{' '}
      <button
        type="button"
        disabled={!draft.editable}
        onClick={() => {
          draft.change((plan) => withMarks(plan, [], null));
        }}
      >
        {t('guided.found.forget')}
      </button>
    </span>
  );
};

/** Whether every group is right, said under the pattern of the time: a new pattern asks again. */
const Confirm = ({ groups }: { readonly groups: number }): ReactElement | null => {
  const { t } = useSimpleText();
  const draft = useDraft();
  if (groups === 0) {
    return null;
  }
  if (readState(draft.plan).groupsOk === draft.plan.grouping.pattern) {
    return <span className="guided-chip is-ok">{t('guided.found.confirmed')}</span>;
  }
  return (
    <button
      type="button"
      className="import-primary"
      disabled={!draft.editable}
      onClick={() => {
        draft.change((plan) => withState(plan, { groupsOk: plan.grouping.pattern }));
      }}
    >
      {t('guided.found.confirm')}
    </button>
  );
};

/** The counts the server made, with the way to each list behind them. */
const Summary = ({ files, items, groups, flags }: { readonly files: number; readonly items: number; readonly groups: number; readonly flags: readonly FlagCount[] }): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const allFiles = { mode: 'simple', screen: 'all-files' } as const;
  const leftOut = flags.filter((flag) => flag.kind === 'not-media' && flag.policy === 'leave').reduce((sum, flag) => sum + flag.count, 0);
  return (
    <div className="guided-facts">
      <span>
        {t('guided.found.fileCount')} <b>{number(files)}</b> <a href={placeHref(draft.id, allFiles)}>{t('guided.found.everyFile')}</a>
      </span>
      <span>
        {t('guided.found.itemCount')} <b>{number(items)}</b>
      </span>
      {leftOut === 0 ? null : (
        <span>
          {t('guided.found.leftOut')} <b>{number(leftOut)}</b> <a href={listHref(draft.id, allFiles, { reason: 'flag' })}>{t('guided.found.showThem')}</a>
        </span>
      )}
      <span>
        {t('guided.found.groupCount')} <b>{number(groups)}</b> <Confirm groups={groups} />
      </span>
      <Rule />
    </div>
  );
};

const Example = ({ record }: { readonly record: RecordView }): ReactElement => {
  const { t, number } = useSimpleText();
  const more = record.files.length - FILES_SHOWN;
  return (
    <li>
      <b>{recordName(record)}</b>
      <ul className="guided-files">
        {record.files.length === 0 ? <li className="muted">{t('lists.groups.noFile')}</li> : null}
        {record.files.slice(0, FILES_SHOWN).map((file) => (
          <li key={file.path} className="guided-file">
            {fileName(file)}
          </li>
        ))}
        {more > 0 ? <li className="muted">{more === 1 ? t('lists.groups.moreOne') : t('lists.groups.more', { count: number(more) })}</li> : null}
      </ul>
    </li>
  );
};

/** One group: the examples the server drew, the way to all of its items, and one change for all of them. */
const Inspector = ({ group, chips, onAnother }: { readonly group: ShapeGroup; readonly chips: readonly Chip[]; readonly onAnother: () => void }): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const load = (page: number, perPage: number) =>
    importApi.preview(draft.id, ['records'], { records: { kind: 'item', shape: group.facts, page, perPage } }).then((preview) => preview.records);
  const several = group.facts.files !== '0' && group.facts.files !== '1';
  const say = group.count === 1 ? t('guided.found.groupOne') : t('guided.found.group', { count: number(group.count) });
  return (
    <section aria-label={say} className="guided-inspect">
      <h4>
        {say} <Chips chips={chips} />
      </h4>
      <ul className="guided-examples">
        {group.samples.map((record) => (
          <Example key={record.id} record={record} />
        ))}
      </ul>
      <p className="import-row-actions">
        {group.count > group.samples.length ? (
          <button type="button" onClick={onAnother}>
            {t('guided.check.another')}
          </button>
        ) : null}
        <a href={listHref(draft.id, { mode: 'simple', screen: 'all-items' }, { group: shapeKey(group.facts) })}>{t('guided.check.showAll', { count: number(group.count) })}</a>
      </p>
      <ChangeMany
        count={group.count}
        noun="items"
        choices={[
          ...(several ? [{ kind: 'button' as const, label: t('guided.check.noFitOwn'), change: { kind: 'move', to: 'own' } as const }] : []),
          { kind: 'button', label: t('guided.check.leave'), change: { kind: 'leave' } },
        ]}
        gather={(onRead, signal) => gather(load, several ? fileEntries : itemEntry, recordName, onRead, signal)}
      />
    </section>
  );
};

const Groups = (): ReactElement => {
  const { t, number } = useSimpleText();
  const draft = useDraft();
  const [page, setPage] = useState(1);
  const [seed, setSeed] = useState(0);
  const read = usePreview(['shapes'], { shapes: { page, perPage: GROUPS_PER_PAGE }, seed });
  if (read.status === 'loading') {
    return <p role="status" className="import-slow">{t('shell.loading')}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const { groups } = read.data.shapes;
  const lastPage = Math.max(1, Math.ceil(groups.total / groups.perPage));
  const rows = groups.items.map((group) => {
    const chips = chipsOf(group, draft.plan, t);
    return { group, chips, key: shapeKey(group.facts), label: named(chips).map((chip) => chip.text).join(', ') };
  });
  return (
    <section aria-busy={read.busy} aria-label={t('lists.groups.label')}>
      <MasterDetail
        label={t('lists.groups.label')}
        initial={null}
        rows={rows.map(({ group, chips, key, label }) => ({
          key,
          // The words a person sees name the decision, so two groups that read alike count once.
          name: (
            <span data-decision={`group:${label}`} aria-label={label}>
              <Chips chips={named(chips)} />
            </span>
          ),
          facts: (
            <>
              <b>{group.count === 1 ? t('guided.found.groupOne') : t('guided.found.group', { count: number(group.count) })}</b>
              {group.odd ? <span className="guided-chip is-warn">{t('guided.found.chipOdd')}</span> : null}
            </>
          ),
        }))}
      >
        {(key) => {
          const row = rows.find((one) => one.key === key);
          return row === undefined ? null : (
            <Inspector
              key={key}
              group={row.group}
              chips={row.chips}
              onAnother={() => {
                // Three samples come with each group; a new seed draws three more.
                setSeed(seed + 1);
              }}
            />
          );
        }}
      </MasterDetail>
      {lastPage > 1 ? <Pager page={page} lastPage={lastPage} total={groups.total} label={t('lists.groups.resource')} onPage={setPage} /> : null}
    </section>
  );
};

const Help = (): ReactElement => {
  const { t } = useSimpleText();
  return (
    <details className="guided-help">
      <summary>{t('guided.list.help')}</summary>
      <p>{t('guided.found.gloss')}</p>
      <p>{t('guided.found.now')}</p>
      <p>{t('guided.found.helpGroups')}</p>
    </details>
  );
};

export const WhatWeFound = (): ReactElement => {
  const { t } = useSimpleText();
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
  return (
    <div className="guided-dials">
      <Summary files={files} items={shapes.records} groups={shapes.groups.total} flags={flags} />
      {listError === null ? null : <ErrorNotice error={listError} />}
      <FilesChart />
      <Groups />
      <Help />
    </div>
  );
};

export default WhatWeFound;
