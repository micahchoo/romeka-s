/**
 * "Show us one example": the person picks one file of an item that has more
 * than one, clicks the part of its name that changes (one click marks it and
 * every part after it, so "page" marks "page 001"), and says what may stand there: only the words they tick,
 * from the words the server found in the same place of the other names, or
 * any number, or, for letters glued to a number (the "a" of "010a"), any
 * letters. Before "Use this", the screen counts what the files would make
 * with the mark, so a wrong mark shows as one huge item before it costs
 * anything.
 *
 * The Structure screen's hand edit places it on one folder (`MarkFor`): it
 * offers that folder's files only, and counts and uses the mark its own way.
 */
import { useState } from 'react';
import type { ComponentType, ReactElement, SyntheticEvent } from 'react';
import type { NameWord, Paged } from '../../../api/import-sections';
import { ErrorNotice } from '../../../app/errors';
import { usePreview } from '../lists/use-preview';
import type { Mark } from '../example';
import { endingOf, markOf, markedText, nameOf, stemParts } from './name-parts';
import { useSimpleText } from './messages';
import type { SimpleTranslator } from './messages';
import './simple.css';

const PICK_READ = 24;
const PICK_SHOWN = 8;
const WORDS_SHOWN = 12;
const ONLY_LETTERS = /^[\p{L}\p{M}]+$/u;

const Search = ({ label, hint, onAsk }: { readonly label: string; readonly hint: string; readonly onAsk: (text: string) => void }): ReactElement => {
  const { t } = useSimpleText();
  const [text, setText] = useState('');
  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    onAsk(text.trim());
  };
  return (
    <form role="search" className="import-filters" onSubmit={submit}>
      <label>
        <span>{label}</span>
        <input type="search" value={text} placeholder={hint} onChange={(event) => { setText(event.target.value); }} />
      </label>
      <button type="submit">{t('guided.example.find')}</button>
    </form>
  );
};

/** The mark and the file it is on: what a trial counts. */
export interface Marked {
  readonly mark: Mark;
  readonly path: string;
}

/** What the marks are for: the files offered, and the trial that counts a mark and uses it. */
export interface MarkFor {
  /** Only the files in this folder or below it; every file when absent. */
  readonly folder?: string;
  readonly Trial: ComponentType<Marked>;
}

const Picker = ({ chosen, folder, onPick }: { readonly chosen: string | null; readonly folder: string | undefined; readonly onPick: (path: string) => void }): ReactElement => {
  const { t } = useSimpleText();
  const [asked, setAsked] = useState('');
  const read = usePreview(['entries'], {
    entries: { used: true, perPage: PICK_READ, ...(folder === undefined ? {} : { folder }), ...(asked === '' ? {} : { text: asked }) },
  });
  const names =
    read.status === 'ready'
      ? read.data.entries.items
          .flatMap((entry) => ('file' in entry.entry && entry.example ? [entry.entry.file.path] : []))
          .filter((path) => stemParts(nameOf(path)).length > 1)
          .slice(0, PICK_SHOWN)
      : [];
  return (
    <>
      <Search label={t('guided.example.findFile')} hint={t('guided.example.findHint')} onAsk={setAsked} />
      {read.status === 'error' ? <ErrorNotice error={read.error} /> : null}
      <fieldset className="guided-opts" aria-label={t('guided.example.fileList')} aria-busy={read.status !== 'ready' || read.busy}>
        {read.status === 'ready' && names.length === 0 ? <p className="muted">{t('guided.example.noName')}</p> : null}
        {names.map((path) => (
          <label key={path}>
            <input type="radio" name="example-file" checked={chosen === path} onChange={() => { onPick(path); }} />
            <span>{nameOf(path)}</span>
          </label>
        ))}
      </fieldset>
    </>
  );
};

const filesWords = (count: number, t: SimpleTranslator['t'], number: SimpleTranslator['number']): string =>
  count === 1 ? t('guided.example.oneFile') : t('guided.example.manyFiles', { count: number(count) });

const Words = ({
  place,
  ticked,
  onTick,
  onAsk,
}: {
  readonly place: Paged<NameWord>;
  readonly ticked: readonly string[];
  readonly onTick: (word: string, on: boolean) => void;
  readonly onAsk: (text: string) => void;
}): ReactElement => {
  const { t, number } = useSimpleText();
  const more = place.total - place.items.length;
  return (
    <>
      <p>
        {t('guided.example.wordsLede')} {t('guided.example.wordsTick')}
      </p>
      {place.total > WORDS_SHOWN ? <Search label={t('guided.example.findWord')} hint="" onAsk={onAsk} /> : null}
      <ul className="guided-ticks">
        {place.items.map((word) => (
          <li key={word.text}>
            <label>
              <input
                type="checkbox"
                checked={ticked.includes(word.text)}
                onChange={(event) => {
                  onTick(word.text, event.target.checked);
                }}
              />{' '}
              {word.text} <span className="muted">{filesWords(word.count, t, number)}</span>
            </label>
          </li>
        ))}
      </ul>
      {more > 0 ? <p className="muted">{t('guided.example.others', { count: number(more) })}</p> : null}
    </>
  );
};

const Marking = ({ path, Trial }: { readonly path: string; readonly Trial: MarkFor['Trial'] }): ReactElement => {
  const { t } = useSimpleText();
  const [selection, setSelection] = useState<number | null>(null);
  const [kind, setKind] = useState<Mark['kind'] | null>(null);
  const [ticked, setTicked] = useState<readonly string[] | null>(null);
  const [wordText, setWordText] = useState('');
  const name = nameOf(path);
  const parts = stemParts(name);
  const ending = endingOf(name);
  const read = usePreview(['nameWords'], { nameWords: { path, perPage: WORDS_SHOWN, ...(wordText === '' ? {} : { text: wordText }) } });
  const pick = (part: number): void => {
    setSelection(selection === part ? null : part);
    setKind(null);
    setTicked(null);
    setWordText('');
  };
  let body: ReactElement | null = null;
  if (selection !== null) {
    const text = markedText(path, selection);
    const glued = parts[selection]?.sep === '';
    // The server's places split only at separators, so a glued part is not one of them.
    const single = !glued && parts.slice(selection + 1).every((part) => part.sep === '');
    const place = parts.slice(1, selection + 1).filter((part) => part.sep !== '').length;
    const digits = /\d/u.test(text);
    const letters = glued && ONLY_LETTERS.test(text);
    const chosenKind = kind ?? (digits ? 'number' : letters ? 'letters' : 'words');
    const words = ticked ?? [text];
    const placeWords = read.status === 'ready' && single ? read.data.nameWords.places[place] : undefined;
    const ready = chosenKind !== 'words' || words.length > 0;
    body = (
      <>
        <fieldset className="guided-opts">
          <legend className="guided-ask-q">{t('guided.example.whatHere')}</legend>
          <label>
            <input type="radio" name="mark-kind" checked={chosenKind === 'words'} onChange={() => { setKind('words'); }} />
            <span>{single ? t('guided.example.onlyTicked') : t('guided.example.onlyThis', { text: `“${text}”` })}</span>
          </label>
          {digits ? (
            <label>
              <input type="radio" name="mark-kind" checked={chosenKind === 'number'} onChange={() => { setKind('number'); }} />
              <span>
                {t('guided.example.anyNumber', { text: `“${text}”` })} <span className="muted">{t('guided.example.hasDigits')}</span>
              </span>
            </label>
          ) : null}
          {letters ? (
            <label>
              <input type="radio" name="mark-kind" checked={chosenKind === 'letters'} onChange={() => { setKind('letters'); }} />
              <span>
                {t('guided.example.anyLetters', { text: `“${text}”` })} <span className="muted">{t('guided.example.afterNumber')}</span>
              </span>
            </label>
          ) : null}
        </fieldset>
        {chosenKind === 'words' && placeWords !== undefined ? (
          <Words
            place={placeWords}
            ticked={words}
            onAsk={setWordText}
            onTick={(word, on) => {
              setTicked(on ? [...words.filter((one) => one !== word), word] : words.filter((one) => one !== word));
            }}
          />
        ) : null}
        {read.status === 'error' ? <ErrorNotice error={read.error} /> : null}
        {ready ? <Trial mark={markOf(path, selection, { kind: chosenKind, words })} path={path} /> : <p className="muted">{t('guided.example.tickOne')}</p>}
      </>
    );
  }
  return (
    <>
      <p>{t('guided.example.parts')}</p>
      <div className="guided-chips" role="group" aria-label={t('guided.example.partsOf', { name })}>
        {parts.map((part, index) => (
          <span key={index}>
            {part.sep === '' ? null : <span className="guided-sep" aria-hidden="true">{part.sep === ' ' ? '␣' : part.sep}</span>}
            <button
              type="button"
              aria-pressed={selection !== null && index >= selection}
              disabled={index === 0}
              onClick={() => {
                pick(index);
              }}
            >
              {part.text === '' ? '∅' : part.text}
            </button>
          </span>
        ))}
        {ending === null ? null : <span className="muted">.{ending}</span>}
      </div>
      <p className="guided-ask-q">{t('guided.example.click')}</p>
      <p className="muted">
        {t('guided.example.clickHint')} {t('guided.example.clickRest')}
      </p>
      {body}
    </>
  );
};

export const ShowUsOneExample = ({ markFor }: { readonly markFor: MarkFor }): ReactElement => {
  const { t } = useSimpleText();
  const [path, setPath] = useState<string | null>(null);
  if (path === null) {
    return (
      <>
        <p className="guided-big">{t('guided.example.choose')}</p>
        <Picker chosen={path} folder={markFor.folder} onPick={setPath} />
      </>
    );
  }
  return (
    <>
      <Marking key={path} path={path} Trial={markFor.Trial} />
      <details>
        <summary>{t('guided.example.other')}</summary>
        <Picker chosen={path} folder={markFor.folder} onPick={setPath} />
      </details>
    </>
  );
};
