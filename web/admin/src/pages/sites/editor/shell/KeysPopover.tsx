/**
 * The toolbar's Keys button and its popover: every key the editor answers,
 * including the ones a card answers only while it has focus.
 */
import type { ReactElement } from 'react';
import { Fragment, useEffect, useRef, useState } from 'react';
import { useTranslator } from '../../../../i18n';
import { EDITOR_NAMESPACES } from '../messages';
import type { EditorMessageKey } from '../messages';

/** One key cap. `mod` is Cmd on a Mac and Ctrl elsewhere. */
type Cap =
  | 'mod'
  | 'shift'
  | 'delete'
  | 'backspace'
  | 'space'
  | 'escape'
  | 'S'
  | 'Z'
  | 'upDown'
  | 'leftRight';

/** Keys pressed together. */
type Chord = readonly Cap[];

interface KeyRow {
  readonly chords: readonly Chord[];
  /** Whether the chords are pressed one after another, not as alternatives. */
  readonly sequence?: true;
  readonly action: EditorMessageKey;
}

const ROWS: readonly KeyRow[] = [
  { chords: [['mod', 'S']], action: 'keys.save' },
  { chords: [['mod', 'Z']], action: 'keys.undo' },
  { chords: [['shift', 'mod', 'Z']], action: 'keys.redo' },
  { chords: [['delete'], ['backspace']], action: 'keys.remove' },
  { chords: [['space'], ['upDown']], sequence: true, action: 'keys.move' },
  { chords: [['shift', 'leftRight']], action: 'keys.resize' },
  { chords: [['escape']], action: 'keys.deselect' },
];

const onMac = (): boolean => /Mac|iPhone|iPad/.test(navigator.userAgent);

export const KeysPopover = (): ReactElement => {
  const { t } = useTranslator(...EDITOR_NAMESPACES);
  const [open, setOpen] = useState(false);
  const wrapper = useRef<HTMLDivElement | null>(null);
  const button = useRef<HTMLButtonElement | null>(null);
  const panel = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!open) {
      return;
    }
    panel.current?.focus();
    const closeOutside = (event: PointerEvent): void => {
      if (!(event.target instanceof Node) || wrapper.current?.contains(event.target) !== true) {
        setOpen(false);
      }
    };
    document.addEventListener('pointerdown', closeOutside);
    return () => {
      document.removeEventListener('pointerdown', closeOutside);
    };
  }, [open]);

  const capText = (cap: Cap): string => {
    switch (cap) {
      case 'mod':
        return onMac() ? '⌘' : t('key.ctrl');
      case 'shift':
        return t('key.shift');
      case 'delete':
        return t('key.delete');
      case 'backspace':
        return t('key.backspace');
      case 'space':
        return t('key.space');
      case 'escape':
        return t('key.escape');
      case 'upDown':
        return '↑ ↓';
      case 'leftRight':
        return '← →';
      case 'S':
      case 'Z':
        return cap;
    }
  };

  return (
    <div
      className="editor-keys"
      ref={wrapper}
      onKeyDown={(event) => {
        if (open && event.key === 'Escape') {
          event.preventDefault();
          setOpen(false);
          button.current?.focus();
        }
      }}
    >
      <button
        type="button"
        ref={button}
        aria-expanded={open}
        aria-controls="editor-keys-panel"
        onClick={() => {
          setOpen(!open);
        }}
      >
        {t('keys.button')}
      </button>
      {open ? (
        <div
          id="editor-keys-panel"
          className="editor-keys-panel"
          role="dialog"
          aria-label={t('keys.title')}
          tabIndex={-1}
          ref={panel}
        >
          <p className="editor-keys-title">{t('keys.title')}</p>
          <dl>
            {ROWS.map((row) => (
              <Fragment key={row.action}>
                <dt>
                  {row.chords.map((chord, index) => (
                    <span key={chord.join('+')} className="editor-keys-chord">
                      {index === 0 ? null : row.sequence ? (
                        ' '
                      ) : (
                        <span aria-hidden="true"> / </span>
                      )}
                      {chord.map((cap, at) => (
                        <Fragment key={cap}>
                          {at > 0 ? '+' : null}
                          <kbd>{capText(cap)}</kbd>
                        </Fragment>
                      ))}
                    </span>
                  ))}
                </dt>
                <dd>{t(row.action)}</dd>
              </Fragment>
            ))}
          </dl>
        </div>
      ) : null}
    </div>
  );
};
