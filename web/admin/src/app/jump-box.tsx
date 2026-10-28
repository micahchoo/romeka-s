/**
 * The header's jump-to box. `#4021`, an identifier or a title, then Enter, opens that record or
 * lists the few that match (`jump.tsx`, its own chunk). `/` puts focus here from anywhere that
 * is not a text field, unless the page took `/` for its own search first (the Items screen
 * does). Escape clears the box and its answer.
 */
import { lazy, Suspense, useCallback, useEffect, useRef, useState } from 'react';
import type { ReactElement, RefObject } from 'react';
import { useTranslator } from '../i18n';

const JumpAnswer = lazy(() => import('./jump'));

/** One press of Enter: `n` makes the same text asked again a new lookup. */
interface Ask {
  readonly text: string;
  readonly n: number;
  /** The address when it was asked: the answer lasts until the reader goes elsewhere. */
  readonly at: string;
}

const isTyping = (target: EventTarget | null): boolean =>
  target instanceof HTMLElement &&
  (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName));

/**
 * `/` focuses the box. The listener is on `window`, which hears a key after `document`, so a
 * page's own `/` (which calls `preventDefault`) wins on that page.
 */
const useSlashFocus = (box: RefObject<HTMLInputElement | null>): void => {
  useEffect(() => {
    const onKey = (event: KeyboardEvent): void => {
      if (
        event.key !== '/' ||
        event.defaultPrevented ||
        event.ctrlKey ||
        event.metaKey ||
        event.altKey ||
        isTyping(event.target)
      ) {
        return;
      }
      event.preventDefault();
      box.current?.focus();
    };
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
    };
  }, [box]);
};

export const JumpBox = (): ReactElement => {
  const { t } = useTranslator();
  const box = useRef<HTMLInputElement>(null);
  const [text, setText] = useState('');
  const [ask, setAsk] = useState<Ask | null>(null);
  useSlashFocus(box);

  const clear = useCallback((): void => {
    setText('');
    setAsk(null);
  }, []);

  // Going elsewhere, the box's own opening included, ends the answer. A `hashchange` arrives
  // after the address changed, so one from a link followed before this ask is no reason.
  useEffect(() => {
    const onHash = (): void => {
      setAsk((current) => (current?.at === window.location.hash ? current : null));
    };
    window.addEventListener('hashchange', onHash);
    return () => {
      window.removeEventListener('hashchange', onHash);
    };
  }, []);

  return (
    <form
      role="search"
      className="jump"
      aria-label={t('jump.label')}
      onSubmit={(event) => {
        event.preventDefault();
        const asked = text.trim();
        if (asked !== '') {
          setAsk((last) => ({ text: asked, n: (last?.n ?? 0) + 1, at: window.location.hash }));
        }
      }}
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          clear();
          box.current?.focus();
        }
      }}
    >
      <input
        ref={box}
        type="search"
        className="jump-input"
        aria-label={t('jump.label')}
        aria-keyshortcuts="/"
        placeholder={t('jump.placeholder')}
        value={text}
        onChange={(event) => {
          setText(event.target.value);
        }}
      />
      <kbd className="jump-key" aria-hidden="true">
        /
      </kbd>
      {ask === null ? null : (
        <Suspense fallback={null}>
          <JumpAnswer key={ask.n} text={ask.text} onDone={clear} />
        </Suspense>
      )}
    </form>
  );
};
