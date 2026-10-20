/**
 * "Show us one example": the person picks one file of an item that has more
 * than one, clicks the part of its name that changes (one click marks it and
 * every part after it, so "page" marks "page 001"), and says what may stand there: only the words they tick,
 * from the words the server found in the same place of the other names, or
 * any number. Before "Use this", the screen counts what the files would make
 * with the mark, so a wrong mark shows as one huge item before it costs
 * anything.
 */
import { useState } from 'react';
import type { ReactElement, SyntheticEvent } from 'react';
import type { Anomaly } from '../../../api/import-model';
import type { BucketCount, NameWord, Paged } from '../../../api/import-sections';
import { ErrorNotice } from '../../../app/errors';
import { useDraft } from '../draft-context';
import { BUCKET_WORDS } from '../lists/facts-words';
import { usePreview } from '../lists/use-preview';
import { placeHref } from '../screens';
import type { Mark } from '../example';
import { endingOf, markOf, markedText, nameOf, stemParts } from './name-parts';
import { readState, withMarks } from './example-state';
import { useSimpleText } from './messages';
import type { SimpleTranslator } from './messages';
import './simple.css';

const PICK_READ = 24;
const PICK_SHOWN = 8;
const WORDS_SHOWN = 12;

const goFound = (id: Parameters<typeof placeHref>[0]): void => {
  window.location.hash = placeHref(id, { mode: 'simple', screen: 'found' });
};

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

const Picker = ({ chosen, onPick }: { readonly chosen: string | null; readonly onPick: (path: string) => void }): ReactElement => {
  const { t } = useSimpleText();
  const [asked, setAsked] = useState('');
  const read = usePreview(['entries'], { entries: { used: true, perPage: PICK_READ, ...(asked === '' ? {} : { text: asked }) } });
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

/** "The biggest has 200 files": a large group names its count; otherwise the largest bucket that has items. */
const biggest = (bars: readonly BucketCount[], anomalies: readonly Anomaly[], text: SimpleTranslator): string => {
  const { t, number } = text;
  const large = anomalies.flatMap((anomaly) => ('largeGroup' in anomaly ? [anomaly.largeGroup.files] : []));
  if (large.length > 0) {
    return filesWords(Math.max(...large), t, number);
  }
  const top = [...bars].reverse().find((bar) => bar.count > 0 && bar.files !== '0');
  if (top === undefined) {
    return filesWords(0, t, number);
  }
  return top.files === '1' || top.files === '2' || top.files === '3' ? filesWords(Number(top.files), t, number) : t(BUCKET_WORDS[top.files]);
};

const Trial = ({ mark, path }: { readonly mark: Mark; readonly path: string }): ReactElement => {
  const text = useSimpleText();
  const { t, number } = text;
  const draft = useDraft();
  const marks = [...readState(draft.plan).marks, mark];
  const read = usePreview(['histogram', 'anomalies'], { plan: withMarks(draft.plan, marks, path) });
  if (read.status === 'loading') {
    return <p role="status" className="import-slow">{t('guided.example.counting')}</p>;
  }
  if (read.status === 'error') {
    return <ErrorNotice error={read.error} />;
  }
  const bars = read.data.histogram.filter((bar) => bar.files !== '0' && bar.count > 0);
  const items = bars.reduce((sum, bar) => sum + bar.count, 0);
  return (
    <div className="guided-ask" aria-busy={read.busy}>
      <p role="status">
        {items === 1 ? t('guided.example.makeOne') : t('guided.example.make', { count: number(items) })}{' '}
        {bars.map((bar) => t('guided.example.with', { count: number(bar.count), files: t(BUCKET_WORDS[bar.files]) })).join(', ')}.{' '}
        {t('guided.example.biggest', { files: biggest(read.data.histogram, read.data.anomalies, text) })}
      </p>
      <p className="import-row-actions">
        <button
          type="button"
          className="import-primary"
          disabled={!draft.editable || read.busy}
          onClick={() => {
            draft.change((plan) => withMarks(plan, [...readState(plan).marks, mark], path));
            goFound(draft.id);
          }}
        >
          {t('guided.example.use')}
        </button>
        <a href={placeHref(draft.id, { mode: 'simple', screen: 'found' })}>{t('guided.example.cancel')}</a>
      </p>
    </div>
  );
};

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
      <p>{t('guided.example.wordsLede')}</p>
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

const Marking = ({ path }: { readonly path: string }): ReactElement => {
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
    const single = selection === parts.length - 1;
    const digits = /\d/u.test(text);
    const chosenKind = kind ?? (digits ? 'number' : 'words');
    const words = ticked ?? [text];
    const place = read.status === 'ready' && single ? read.data.nameWords.places[selection] : undefined;
    const ready = chosenKind === 'number' || words.length > 0;
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
        </fieldset>
        {chosenKind === 'words' && place !== undefined ? (
          <Words
            place={place}
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
            {index === 0 ? null : <span className="guided-sep" aria-hidden="true">{part.sep === ' ' ? '␣' : part.sep}</span>}
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
      <p className="muted">{t('guided.example.clickHint')}</p>
      {body}
    </>
  );
};

export const ShowUsOneExample = (): ReactElement => {
  const { t } = useSimpleText();
  const [path, setPath] = useState<string | null>(null);
  if (path === null) {
    return (
      <>
        <p className="guided-big">{t('guided.example.choose')}</p>
        <Picker chosen={path} onPick={setPath} />
      </>
    );
  }
  return (
    <>
      <Marking key={path} path={path} />
      <details>
        <summary>{t('guided.example.other')}</summary>
        <Picker chosen={path} onPick={setPath} />
      </details>
    </>
  );
};

export default ShowUsOneExample;
