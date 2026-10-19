/**
 * The Tags panel of the Term lists page: every tag with how many resources
 * carry it, the merge prompt for tags that look alike, and "Promote to list",
 * which turns chosen tags into the words of a list after showing what would
 * change.
 */
import type { ReactElement } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { getCensus, promoteTags } from '../../api/tags';
import type { Census, Promotion } from '../../api/tags';
import { listTermLists } from '../../api/term-lists';
import { ErrorNotice } from '../../app/errors';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { MergePrompt } from './merge-prompt';
import { useTermListText } from './messages';
import { RewriteCounts } from './rewrite-counts';
import { similarGroups } from './similar';
import './term-lists.css';

const TAGS = { kind: 'tags' } as const;

interface Loaded {
  readonly census: Census;
  /** Labels of the lists that hold words, which a promotion can add to. */
  readonly wordLists: readonly string[];
}

const load = async (): Promise<Loaded> => {
  const [census, lists] = await Promise.all([
    getCensus(api, TAGS),
    listTermLists(api, { perPage: 200, sortBy: 'label' }),
  ]);
  return {
    census,
    wordLists: lists.items
      .filter((list) => list.source.kind === 'terms' || list.source.kind === 'empty')
      .map((list) => list.label),
  };
};

type Step =
  | { readonly kind: 'choose' }
  | { readonly kind: 'busy' }
  | { readonly kind: 'confirm'; readonly preview: Promotion }
  | { readonly kind: 'error'; readonly error: Error };

const asError = (error: unknown): Error =>
  error instanceof Error ? error : new Error(String(error));

export const TagsPanel = ({ onPromoted }: { readonly onPromoted: () => void }): ReactElement => {
  const { t } = useTranslator();
  const tl = useTermListText();
  const [version, setVersion] = useState(0);
  const state = useAsync(load, [version]);
  const [chosen, setChosen] = useState<readonly string[]>([]);
  const [listName, setListName] = useState('');
  const [step, setStep] = useState<Step>({ kind: 'choose' });
  const [notice, setNotice] = useState('');

  const reload = (message: string): void => {
    setNotice(message);
    setVersion((current) => current + 1);
  };
  const run = (task: () => Promise<void>): void => {
    setStep({ kind: 'busy' });
    task().catch((error: unknown) => {
      setStep({ kind: 'error', error: asError(error) });
    });
  };
  const write = (dryRun: boolean): Promise<Promotion> =>
    promoteTags(api, { tags: chosen, list: listName.trim(), dryRun });
  const preview = (): void => {
    run(async () => {
      setStep({ kind: 'confirm', preview: await write(true) });
    });
  };
  const promote = (): void => {
    run(async () => {
      const done = await write(false);
      setChosen([]);
      setListName('');
      setStep({ kind: 'choose' });
      reload(tl('tags.promoted', { list: done.list?.label ?? listName, changed: done.changed }));
      onPromoted();
    });
  };

  const body = (): ReactElement => {
    if (state.status === 'loading') {
      return <p role="status">{t('loading.form')}</p>;
    }
    if (state.status === 'error') {
      return <ErrorNotice error={state.error} />;
    }
    const { census, wordLists } = state.data;
    if (census.values.length === 0) {
      return <p>{tl('tags.empty')}</p>;
    }
    const groups = similarGroups(census.values, census.distinct);
    const ready = chosen.length > 0 && listName.trim() !== '' && step.kind !== 'busy';
    return (
      <>
        {groups.map((group) => (
          <MergePrompt
            key={`${String(version)}:${group.map((count) => count.value).join('|')}`}
            scope={TAGS}
            group={group}
            onDone={reload}
          />
        ))}
        <fieldset className="tag-choices" disabled={step.kind === 'busy'}>
          <legend>{tl('tags.legend')}</legend>
          {census.values.map((count) => (
            <label key={count.value}>
              <input
                type="checkbox"
                checked={chosen.includes(count.value)}
                onChange={(event) => {
                  const checked = event.target.checked;
                  setChosen((current) =>
                    checked
                      ? [...current, count.value]
                      : current.filter((value) => value !== count.value),
                  );
                  setStep({ kind: 'choose' });
                }}
              />
              <span className="tag-choice-name">{count.value}</span>
              <span className="tag-choice-count">{count.resources}</span>
            </label>
          ))}
        </fieldset>
        <p className="tag-list-name">
          <label htmlFor="tags-list-name">{tl('tags.listName')}</label>
          <input
            id="tags-list-name"
            type="text"
            list="tags-word-lists"
            autoComplete="off"
            aria-describedby="tags-list-name-hint"
            value={listName}
            onChange={(event) => {
              setListName(event.target.value);
              setStep({ kind: 'choose' });
            }}
          />
          <datalist id="tags-word-lists">
            {wordLists.map((label) => (
              <option key={label} value={label} />
            ))}
          </datalist>
          <span id="tags-list-name-hint" className="field-hint">
            {tl('tags.listHint')}
          </span>
        </p>
        {step.kind === 'confirm' ? (
          <PromotionPreview
            preview={step.preview}
            list={listName.trim()}
            onConfirm={promote}
            onCancel={() => {
              setStep({ kind: 'choose' });
            }}
          />
        ) : (
          <p>
            <button type="button" disabled={!ready} onClick={preview}>
              {tl('tags.promote')}
            </button>
          </p>
        )}
        {step.kind === 'error' ? <ErrorNotice error={step.error} /> : null}
      </>
    );
  };

  return (
    <aside className="tags-panel" aria-labelledby="tags-panel-title">
      <h2 id="tags-panel-title">{tl('tags.title')}</h2>
      <p className="field-hint">{tl('tags.intro')}</p>
      <p role="status">{notice}</p>
      {body()}
    </aside>
  );
};

const PromotionPreview = ({
  preview,
  list,
  onConfirm,
  onCancel,
}: {
  readonly preview: Promotion;
  readonly list: string;
  readonly onConfirm: () => void;
  readonly onCancel: () => void;
}): ReactElement => {
  const { t } = useTranslator();
  const tl = useTermListText();
  const names = (named: readonly { readonly label: string }[]): string =>
    named.length === 0 ? tl('rewrite.none') : named.map((entry) => entry.label).join(', ');
  return (
    <section className="promote-preview" aria-labelledby="promote-preview-title">
      <h3 id="promote-preview-title">{tl('tags.promoteTitle', { list })}</h3>
      {preview.created ? <p>{tl('tags.newList')}</p> : null}
      <RewriteCounts rewrite={preview} skipped="tags.skipped" />
      <dl className="rewrite-counts">
        <dt>{tl('tags.fields')}</dt>
        <dd>{names(preview.templates)}</dd>
        {preview.templatesSkipped.length === 0 ? null : (
          <>
            <dt>{tl('tags.fieldsSkipped')}</dt>
            <dd>{names(preview.templatesSkipped)}</dd>
          </>
        )}
      </dl>
      <p className="merge-actions">
        <button type="button" className="button-primary" onClick={onConfirm}>
          {tl('tags.promote')}
        </button>
        <button type="button" onClick={onCancel}>
          {t('action.cancel')}
        </button>
      </p>
    </section>
  );
};
