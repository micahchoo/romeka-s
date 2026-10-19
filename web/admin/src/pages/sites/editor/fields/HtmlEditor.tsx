/**
 * The `html` field control: a small rich-text editor over one `contentEditable`
 * element, with a toolbar and a source view. No dependency; the server purifies
 * whatever HTML this produces (`crates/omeka-site/src/blocks/html.rs`).
 *
 * A toolbar command wraps the current selection, taken from the live selection
 * when it sits inside the editor and from the range saved when the Link control
 * took focus otherwise. The commands use the DOM Range API rather than
 * `document.execCommand`, which jsdom does not implement and which browsers
 * deprecate.
 */
import type { ReactElement } from 'react';
import { useEffect, useRef, useState } from 'react';
import { useTranslator } from '../../../../i18n';
import { common } from '../../../../i18n/namespaces/common';
import { sites } from '../../../../i18n/namespaces/sites';

export interface HtmlEditorProps {
  readonly id: string;
  readonly label: string;
  readonly value: string;
  readonly onChange: (html: string) => void;
  readonly required?: boolean;
}

type Wrap = (range: Range) => Element | null;

const makeElement = (tag: string, text?: string): Element => {
  const node = document.createElement(tag);
  if (text !== undefined) {
    node.textContent = text;
  }
  return node;
};

const makeList = (tag: 'ul' | 'ol'): Element => {
  const wrapper = makeElement(tag);
  wrapper.appendChild(makeElement('li'));
  return wrapper;
};

/** Replaces the range's contents with `node` and puts the caret after it. */
const wrapRange = (target: Range, node: Element): void => {
  node.appendChild(target.extractContents());
  target.insertNode(node);
  target.setStartAfter(node);
  target.collapse(true);
  const selection = window.getSelection();
  selection?.removeAllRanges();
  selection?.addRange(target);
};

export const HtmlEditor = ({
  id,
  label,
  value,
  onChange,
  required = false,
}: HtmlEditorProps): ReactElement => {
  const { t } = useTranslator(common, sites);
  const editor = useRef<HTMLDivElement | null>(null);
  const saved = useRef<Range | null>(null);
  const [source, setSource] = useState(false);
  const [link, setLink] = useState<string | null>(null);

  useEffect(() => {
    const element = editor.current;
    if (element !== null && element.innerHTML !== value) {
      element.innerHTML = value;
    }
  }, [value, source]);

  /** The range a command acts on: the live selection inside the editor, else the saved one. */
  const range = (): Range | null => {
    const element = editor.current;
    const selection = window.getSelection();
    if (element !== null && selection !== null && selection.rangeCount > 0) {
      const live = selection.getRangeAt(0);
      if (element.contains(live.commonAncestorContainer)) {
        return live;
      }
    }
    return saved.current;
  };

  const wrap = (make: Wrap): void => {
    const element = editor.current;
    const target = range();
    if (element === null || target === null) {
      return;
    }
    const node = make(target);
    if (node === null) {
      return;
    }
    wrapRange(target, node);
    onChange(element.innerHTML);
  };

  const commands: readonly { readonly label: string; readonly run: () => void }[] = [
    { label: t('html.bold'), run: () => { wrap(() => makeElement('strong')); } },
    { label: t('html.italic'), run: () => { wrap(() => makeElement('em')); } },
    {
      label: t('html.link'),
      run: () => {
        saved.current = range();
        setLink('');
      },
    },
    { label: t('html.heading2'), run: () => { wrap(() => makeElement('h2')); } },
    { label: t('html.heading3'), run: () => { wrap(() => makeElement('h3')); } },
    { label: t('html.bulleted'), run: () => { wrap(() => makeList('ul')); } },
    { label: t('html.numbered'), run: () => { wrap(() => makeList('ol')); } },
  ];

  const applyLink = (): void => {
    const url = link ?? '';
    if (url.trim() !== '') {
      wrap(() => {
        const anchor = document.createElement('a');
        anchor.setAttribute('href', url.trim());
        return anchor;
      });
    }
    setLink(null);
  };

  return (
    <div className="block-field html-field">
      <label htmlFor={id}>{label}</label>
      <div className="html-toolbar" role="toolbar" aria-label={t('html.toolbar')}>
        {commands.map((command) => (
          <button
            key={command.label}
            type="button"
            onMouseDown={(event) => {
              event.preventDefault();
            }}
            onClick={command.run}
          >
            {command.label}
          </button>
        ))}
        <button
          type="button"
          aria-pressed={source}
          onClick={() => {
            setSource(!source);
          }}
        >
          {t('html.source')}
        </button>
      </div>
      {link === null ? null : (
        <p className="html-link-form">
          <label htmlFor={`${id}-url`}>{t('html.url')}</label>
          <input
            id={`${id}-url`}
            type="url"
            value={link}
            onChange={(event) => {
              setLink(event.target.value);
            }}
          />
          <button type="button" onClick={applyLink}>
            {t('html.apply')}
          </button>
          <button
            type="button"
            onClick={() => {
              setLink(null);
            }}
          >
            {t('action.cancel')}
          </button>
        </p>
      )}
      {source ? (
        <textarea
          id={id}
          aria-label={label}
          rows={6}
          value={value}
          onChange={(event) => {
            onChange(event.target.value);
          }}
        />
      ) : (
        <div
          id={id}
          ref={editor}
          className="html-editor"
          role="textbox"
          aria-label={label}
          aria-multiline="true"
          aria-required={required}
          data-placeholder={t('html.empty')}
          contentEditable
          suppressContentEditableWarning
          onInput={(event) => {
            onChange(event.currentTarget.innerHTML);
          }}
        />
      )}
    </div>
  );
};
