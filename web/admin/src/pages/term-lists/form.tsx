/**
 * `CustomVocab\Controller\Admin\IndexController::addAction` and `::editAction`:
 * one term list, the kind it holds and its entries.
 *
 * The form keeps one draft per kind, so switching the kind and back does not
 * lose what was typed; only the chosen kind is sent.
 */
import type { ReactElement, SyntheticEvent } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import { createTermList, deleteTermList, getTermList, updateTermList } from '../../api/term-lists';
import type {
  ItemSet,
  ItemSetId,
  TermList,
  TermListId,
  TermListUri,
  TermListWrite,
} from '../../api/types';
import { DeleteControl } from '../../app/delete';
import { ErrorNotice } from '../../app/errors';
import { SaveStatus, SubmitRow, TextField } from '../../app/fields';
import { useSave } from '../../app/save';
import { useAsync } from '../../app/useAsync';
import { useTranslator } from '../../i18n';
import { useTermListText } from './messages';
import { KIND_LABEL } from './list';
import { ListMerges } from './list-merges';

type Kind = TermListWrite['source']['kind'];

const KINDS: readonly Kind[] = ['terms', 'items', 'uris'];

const linesOf = (text: string): readonly string[] =>
  text
    .split(/\r\n|\r|\n/)
    .map((line) => line.trim())
    .filter((line) => line !== '');

/** One line per link, the way the module's form reads them: URI, space, label. */
const urisOf = (text: string): readonly TermListUri[] =>
  linesOf(text).map((line) => {
    const space = line.indexOf(' ');
    return space === -1
      ? { uri: line, label: '' }
      : { uri: line.slice(0, space), label: line.slice(space + 1).trim() };
  });

const urisText = (uris: readonly TermListUri[]): string =>
  uris.map((entry) => `${entry.uri} ${entry.label}`.trim()).join('\n');

const TermListEditor = ({
  list,
  itemSets,
  onSaved,
}: {
  readonly list: TermList | null;
  readonly itemSets: readonly ItemSet[];
  readonly onSaved: (list: TermList) => void;
}): ReactElement => {
  const { t } = useTranslator();
  const tl = useTermListText();
  const source = list?.source ?? { kind: 'empty' };
  const [label, setLabel] = useState(list?.label ?? '');
  const [lang, setLang] = useState(list?.lang ?? '');
  const [kind, setKind] = useState<Kind>(source.kind === 'empty' ? 'terms' : source.kind);
  const [terms, setTerms] = useState(source.kind === 'terms' ? source.terms.join('\n') : '');
  const [uris, setUris] = useState(source.kind === 'uris' ? urisText(source.uris) : '');
  const [itemSet, setItemSet] = useState<ItemSetId | null>(
    source.kind === 'items' ? source.itemSet.id : null,
  );
  const [formError, setFormError] = useState<string | null>(null);
  const { state: saveState, save } = useSave();

  const sourceWrite = (): TermListWrite['source'] | string => {
    switch (kind) {
      case 'terms': {
        const entries = linesOf(terms);
        return entries.length === 0 ? tl('termLists.errorNoEntries') : { kind, terms: entries };
      }
      case 'uris': {
        const entries = urisOf(uris);
        return entries.length === 0 ? tl('termLists.errorNoEntries') : { kind, uris: entries };
      }
      case 'items':
        return itemSet === null ? tl('termLists.errorNoItemSet') : { kind, itemSet };
    }
  };

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (label.trim() === '') {
      setFormError(t('form.errorEmptyLabel'));
      return;
    }
    const chosen = sourceWrite();
    if (typeof chosen === 'string') {
      setFormError(chosen);
      return;
    }
    setFormError(null);
    const write: TermListWrite = {
      label,
      lang: lang.trim() === '' ? null : lang.trim(),
      source: chosen,
    };
    save(async () => {
      if (list === null) {
        await createTermList(api, write);
        window.location.hash = '#/term-lists';
      } else {
        onSaved(await updateTermList(api, list.id, write));
      }
    });
  };

  return (
    <form onSubmit={submit}>
      <fieldset>
        <legend>{tl('termLists.legend')}</legend>
        <TextField id="term-list-label" label={t('column.label')} value={label} onChange={setLabel} />
      </fieldset>
      <fieldset className="term-list-kinds">
        <legend>{tl('termLists.kindLegend')}</legend>
        {KINDS.map((entry) => (
          <label key={entry}>
            <input
              type="radio"
              name="term-list-kind"
              value={entry}
              checked={kind === entry}
              onChange={() => {
                setKind(entry);
              }}
            />
            {tl(KIND_LABEL[entry])}
          </label>
        ))}
        {kind === 'terms' ? (
          <p className="value-field-wide">
            <label htmlFor="term-list-terms">{tl('termLists.termsLabel')}</label>
            <textarea
              id="term-list-terms"
              rows={10}
              value={terms}
              onChange={(event) => {
                setTerms(event.target.value);
              }}
            />
          </p>
        ) : null}
        {kind === 'uris' ? (
          <p className="value-field-wide">
            <label htmlFor="term-list-uris">{tl('termLists.urisLabel')}</label>
            <textarea
              id="term-list-uris"
              rows={10}
              value={uris}
              onChange={(event) => {
                setUris(event.target.value);
              }}
            />
          </p>
        ) : null}
        {kind === 'items' ? (
          <p>
            <label htmlFor="term-list-item-set">{tl('termLists.itemSetLabel')}</label>
            <select
              id="term-list-item-set"
              value={itemSet === null ? '' : String(itemSet)}
              onChange={(event) => {
                setItemSet(
                  itemSets.find((set) => String(set.id) === event.target.value)?.id ?? null,
                );
              }}
            >
              <option value="">{tl('termLists.selectItemSet')}</option>
              {itemSets.map((set) => (
                <option key={String(set.id)} value={String(set.id)}>
                  {set.title ?? t('common.untitled')}
                </option>
              ))}
            </select>
          </p>
        ) : null}
        {/* `CustomVocab::hydrate` gives the list's language to words only. */}
        {kind === 'terms' ? (
          <p>
            <label htmlFor="term-list-lang">{tl('termLists.langLabel')}</label>
            <input
              id="term-list-lang"
              type="text"
              aria-describedby="term-list-lang-hint"
              value={lang}
              onChange={(event) => {
                setLang(event.target.value);
              }}
            />
            <span id="term-list-lang-hint" className="field-hint">
              {tl('termLists.langHint')}
            </span>
          </p>
        ) : null}
      </fieldset>
      {formError === null ? null : <p role="alert">{formError}</p>}
      <SaveStatus state={saveState} />
      <SubmitRow
        label={list === null ? tl('termLists.add') : t('action.save')}
        busy={saveState.status === 'saving'}
      />
    </form>
  );
};

const loadItemSets = async (): Promise<readonly ItemSet[]> =>
  (await api.listItemSets({ perPage: 200, sortBy: 'title' })).items;

/** `#/term-lists/new`. */
export const TermListCreatePage = (): ReactElement => {
  const { t } = useTranslator();
  const state = useAsync(loadItemSets, []);
  if (state.status === 'loading') {
    return <p role="status">{t('loading.form')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  return <TermListEditor list={null} itemSets={state.data} onSaved={() => undefined} />;
};

interface Loaded {
  readonly list: TermList;
  readonly itemSets: readonly ItemSet[];
}

/** `#/term-lists/<id>`: the edit form and the delete control. */
export const TermListEditPage = ({ id }: { readonly id: TermListId }): ReactElement => {
  const { t } = useTranslator();
  const tl = useTermListText();
  const state = useAsync(
    async (): Promise<Loaded> => {
      const [list, itemSets] = await Promise.all([getTermList(api, id), loadItemSets()]);
      return { list, itemSets };
    },
    [id],
  );
  const [saved, setSaved] = useState<TermList | null>(null);
  const [merged, setMerged] = useState({ version: 0, message: '' });
  if (state.status === 'loading') {
    return <p role="status">{t('loading.form')}</p>;
  }
  if (state.status === 'error') {
    return <ErrorNotice error={state.error} />;
  }
  const list = saved ?? state.data.list;
  return (
    <div>
      <h2>{list.label}</h2>
      <p>
        <a href="#/term-lists">{t('page.termLists')}</a>
      </p>
      <p role="status">{merged.message}</p>
      <ListMerges
        list={list}
        onMerged={(message) => {
          const shown = (): void => {
            setMerged((current) => ({ version: current.version + 1, message }));
          };
          getTermList(api, id).then((fresh) => {
            setSaved(fresh);
            shown();
          }, shown);
        }}
      />
      <TermListEditor
        key={merged.version}
        list={list}
        itemSets={state.data.itemSets}
        onSaved={setSaved}
      />
      <p>{tl('termLists.deleteNote')}</p>
      <DeleteControl
        label={tl('termLists.deleteLabel')}
        rowName={list.label}
        onConfirm={async () => {
          await deleteTermList(api, list.id);
          window.location.hash = '#/term-lists';
        }}
      />
    </div>
  );
};
