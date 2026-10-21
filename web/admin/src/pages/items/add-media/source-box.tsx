/**
 * The panel's one source box: choose files, drop them, or paste a link. A
 * pasted link is read by the server before anything saves, so the button names
 * what it will add.
 *
 * A file starts sending the moment it is picked; a link waits for the button,
 * because the server's answer decides which reader is used.
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import type { DragEvent, ReactElement } from 'react';
import { useTranslator } from '../../../i18n';
import type { KeyOf, Translator } from '../../../i18n';
import { mediaKindOfServerKind } from '../../../app/media';
import { sizeText } from '../../../app/format';
import { fromBrowserEntry, fromDrop, fromInput } from '../../../app/upload/pick';
import { PROBE_INGESTERS, probeLink } from '../../../api/staged-uploads';
import type { Ingester, Probe } from '../../../api/staged-uploads';
import type { AddStore, AddsSnapshot } from './store';
import { INGESTER_WORD, KINDS_WORD, READING_WORD, SUBMIT_LABEL } from './ingesters';
import { mediaAdd } from '../../../i18n/namespaces/media-add';
import { resources } from '../../../i18n/namespaces/resources';

/** The file input every "Choose files" control points at. */
export const FILE_INPUT_ID = 'add-media-files';

type PanelTranslator = Translator<KeyOf<typeof mediaAdd> | KeyOf<typeof resources>>;

type ProbeState =
  | { readonly kind: 'idle' }
  | { readonly kind: 'probing'; readonly url: string }
  | { readonly kind: 'read'; readonly url: string; readonly probe: Probe }
  | { readonly kind: 'failed'; readonly url: string; readonly reason: string };

const PROBE_DEBOUNCE_MS = 400;
const SLOW_MS = 1000;

/** A flag that turns on once `active` has lasted a second, for a waiting line. */
const useSlow = (active: boolean): boolean => {
  const [slow, setSlow] = useState(false);
  useEffect(() => {
    if (!active) {
      setSlow(false);
      return;
    }
    const timer = setTimeout(() => {
      setSlow(true);
    }, SLOW_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [active]);
  return slow;
};

const takesWords = (snapshot: AddsSnapshot, translator: PanelTranslator): string => {
  if (snapshot.limits.kind !== 'ready') {
    return '';
  }
  const { t, locale } = translator;
  const kinds = new Set(
    snapshot.limits.limits.kinds.flatMap((kind) => [mediaKindOfServerKind(kind) ?? 'other']),
  );
  const words = [...kinds].map((kind) => t(KINDS_WORD[kind]));
  return new Intl.ListFormat(locale, { style: 'long', type: 'conjunction' }).format(words);
};

const readingLine = (probe: Probe, translator: PanelTranslator): string => {
  const { t, locale } = translator;
  const reading = t(READING_WORD[probe.suggested]);
  const detail =
    probe.title ??
    (probe.image === null ? null : t('add.detailImage', { width: probe.image.w, height: probe.image.h })) ??
    (probe.canvases === null ? null : t('add.detailCanvases', { count: probe.canvases })) ??
    (probe.size === null ? null : t('add.detailSize', { size: sizeText(probe.size, locale) })) ??
    probe.mediaType;
  return detail === null || detail === '' ? reading : t('add.readingWith', { reading, detail });
};

export const SourceBox = ({
  store,
  snapshot,
  advanced,
  onAdvancedChange,
}: {
  readonly store: AddStore;
  readonly snapshot: AddsSnapshot;
  readonly advanced: boolean;
  readonly onAdvancedChange: (advanced: boolean) => void;
}): ReactElement => {
  const translator = useTranslator(mediaAdd, resources);
  const { t, locale } = translator;
  const [text, setText] = useState('');
  const [html, setHtml] = useState('');
  const [reader, setReader] = useState<Ingester | null>(null);
  const [probe, setProbe] = useState<ProbeState>({ kind: 'idle' });
  const [over, setOver] = useState(false);
  const [reading, setReading] = useState(false);
  const lastRun = useRef<string | null>(null);
  const seq = useRef(0);
  const depth = useRef(0);
  const input = useRef<HTMLInputElement>(null);
  const probing = probe.kind === 'probing';
  const slow = useSlow(probing);
  const shownProbe = probe.kind === 'read' || probe.kind === 'failed' ? (probe.url === text.trim() ? probe : null) : null;

  const run = useCallback(async (raw: string): Promise<void> => {
    const url = raw.trim();
    if (url === '') {
      setProbe({ kind: 'idle' });
      return;
    }
    lastRun.current = url;
    seq.current += 1;
    const mine = seq.current;
    setProbe({ kind: 'probing', url });
    try {
      const answer = await probeLink(url);
      if (seq.current === mine) {
        setProbe({ kind: 'read', url, probe: answer });
      }
    } catch (error) {
      if (seq.current === mine) {
        setProbe({ kind: 'failed', url, reason: error instanceof Error ? error.message : String(error) });
      }
    }
  }, []);

  useEffect(() => {
    const url = text.trim();
    if (url === '') {
      lastRun.current = null;
      setProbe({ kind: 'idle' });
      return;
    }
    if (lastRun.current === url) {
      return;
    }
    const timer = setTimeout(() => {
      void run(url);
    }, PROBE_DEBOUNCE_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [text, run]);

  const answer = shownProbe?.kind === 'read' ? shownProbe.probe : null;
  const plain = shownProbe?.kind === 'failed';
  const ingester: Ingester = reader ?? answer?.suggested ?? 'url';
  const htmlReady = html.trim() !== '';
  const linkReady = text.trim() !== '' && (answer !== null || plain);
  const submitLabel = htmlReady ? t('submit.html') : answer !== null ? t(SUBMIT_LABEL[ingester]) : plain ? t('submit.plain') : t('submit.link');

  const submit = (): void => {
    if (htmlReady) {
      store.addLink({ kind: 'html', ingester: 'html', text: html.trim(), label: t('ingester.html') });
      setHtml('');
      return;
    }
    if (!linkReady) {
      return;
    }
    store.addLink({ kind: 'link', ingester, text: text.trim(), label: text.trim() });
    setText('');
    setReader(null);
    setProbe({ kind: 'idle' });
    lastRun.current = null;
  };

  const addDrop = (event: DragEvent<HTMLDivElement>): void => {
    event.preventDefault();
    depth.current = 0;
    setOver(false);
    // The items are readable only inside this handler.
    const entries = Array.from(event.dataTransfer.items, (entry) => (entry.kind === 'file' ? fromBrowserEntry(entry.webkitGetAsEntry()) : null));
    const files = Array.from(event.dataTransfer.files);
    setReading(true);
    void fromDrop(entries, files)
      .then((picked) => {
        store.addPicked(picked);
      })
      .finally(() => {
        setReading(false);
      });
  };

  const takes = takesWords(snapshot, translator);

  return (
    <div className="add-source">
      <div
        className="add-drop"
        data-over={over}
        onDragEnter={(event) => {
          event.preventDefault();
          depth.current += 1;
          setOver(true);
        }}
        onDragOver={(event) => {
          event.preventDefault();
          event.dataTransfer.dropEffect = 'copy';
        }}
        onDragLeave={() => {
          depth.current -= 1;
          if (depth.current <= 0) {
            depth.current = 0;
            setOver(false);
          }
        }}
        onDrop={addDrop}
      >
        <p className="add-drop-hint">{reading ? t('add.reading') : t('add.dropHint')}</p>
        <button type="button" autoFocus onClick={() => { input.current?.click(); }}>
          {t('add.chooseFiles')}
        </button>
        <input
          ref={input}
          id={FILE_INPUT_ID}
          type="file"
          multiple
          hidden
          tabIndex={-1}
          onChange={(event) => {
            const files = event.currentTarget.files;
            if (files !== null) {
              store.addPicked(fromInput(files));
            }
            event.currentTarget.value = '';
          }}
        />
      </div>

      <p className="add-link">
        <label htmlFor="add-media-link">{t('add.linkLabel')}</label>
        <input
          id="add-media-link"
          type="url"
          inputMode="url"
          placeholder={t('add.linkPlaceholder')}
          value={text}
          onChange={(event) => {
            setText(event.currentTarget.value);
          }}
          onPaste={(event) => {
            const pasted = event.clipboardData.getData('text');
            if (pasted !== '') {
              event.preventDefault();
              const next = pasted.trim();
              setText(next);
              void run(next);
            }
          }}
        />
        <button type="button" disabled={!(htmlReady || linkReady)} onClick={submit}>
          {submitLabel}
        </button>
      </p>

      {shownProbe === null ? null : (
        <p className="add-reading" role="status">
          {shownProbe.kind === 'read' ? readingLine(shownProbe.probe, translator) : t('add.probeFailed', { reason: shownProbe.reason })}
        </p>
      )}
      {probing && slow ? <p className="add-hint" role="status">{t('add.probing')}</p> : null}
      {over && !reading ? <p className="add-hint">{t('add.dropNow')}</p> : null}

      <p className="add-links">
        <button type="button" aria-expanded={advanced} onClick={() => { onAdvancedChange(!advanced); }}>
          {advanced ? t('add.simple') : t('add.advanced')}
        </button>
      </p>

      {advanced ? (
        <div className="add-advanced">
          <p>
            <label htmlFor="add-media-html">{t('add.html')}</label>
            <textarea
              id="add-media-html"
              rows={3}
              value={html}
              onChange={(event) => {
                setHtml(event.currentTarget.value);
              }}
            />
            <span className="add-hint">{t('add.htmlNote')}</span>
          </p>
          <p>
            <label htmlFor="add-media-reader">{t('add.readAs')}</label>
            <select
              id="add-media-reader"
              value={ingester}
              onChange={(event) => {
                const value = PROBE_INGESTERS.find((candidate) => candidate === event.currentTarget.value);
                setReader(value ?? null);
              }}
            >
              {(answer?.readers ?? PROBE_INGESTERS).map((candidate) => (
                <option key={candidate} value={candidate}>{t(INGESTER_WORD[candidate])}</option>
              ))}
            </select>
          </p>
          <p>
            <label>
              <input
                type="checkbox"
                checked={snapshot.isPublic}
                onChange={(event) => {
                  store.setPublic(event.currentTarget.checked);
                }}
              />
              {' '}
              {t('add.visible')}
            </label>
          </p>
        </div>
      ) : null}

      {snapshot.limits.kind === 'loading' ? (
        <p className="add-hint">{t('add.checking')}</p>
      ) : snapshot.limits.kind === 'failed' ? (
        <p className="add-hint">{t('add.limitsFailed')}</p>
      ) : (
        <p className="add-hint">{t('add.takes', { kinds: takes, size: sizeText(snapshot.limits.limits.maxFileBytes, locale) })}</p>
      )}
    </div>
  );
};
