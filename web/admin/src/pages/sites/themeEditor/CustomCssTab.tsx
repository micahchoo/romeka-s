/**
 * The Custom CSS tab, loaded on demand: a monospace editor with line numbers
 * and the sanitiser's errors marked by line, beside a read-only view of the
 * CSS the theme generates, so a curator can see what there is to override.
 *
 * The editor is a textarea with a gutter rather than a code-editor library:
 * the lightest such library is several times the 10 KB a lazy chunk may
 * weigh (`perf/budget.json`), and the sanitiser's errors are what a curator
 * needs most, which the gutter shows.
 */
import type { KeyboardEvent, ReactElement } from 'react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { defaultBaseUrl } from '../../../api/client';
import { useThemeText } from './messages';
import type { ThemeMessageKey } from './messages';

/** The line a sanitiser message names, `Line 3, column 5: ...`. */
const lineOf = (message: string): number | null => {
  const match = /Line (\d+), column \d+/.exec(message);
  return match?.[1] === undefined ? null : Number(match[1]);
};

const byteLength = (text: string): number => new TextEncoder().encode(text).length;

const CssEditor = ({
  value,
  maxBytes,
  errors,
  onChange,
}: {
  readonly value: string;
  readonly maxBytes: number;
  readonly errors: readonly string[];
  readonly onChange: (value: string) => void;
}): ReactElement => {
  const t = useThemeText();
  const area = useRef<HTMLTextAreaElement | null>(null);
  const gutter = useRef<HTMLPreElement | null>(null);
  const badLines = useMemo(
    () => new Set(errors.map(lineOf).filter((line): line is number => line !== null)),
    [errors],
  );
  const lines = value.split('\n').length;

  // Tab indents rather than leaving the editor, as in any code editor;
  // Escape then Tab still leaves it, so the keyboard is never trapped.
  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>): void => {
    if (event.key !== 'Tab' || event.shiftKey || event.currentTarget.dataset['escaped'] === 'true') {
      if (event.key === 'Escape') {
        event.currentTarget.dataset['escaped'] = 'true';
      }
      return;
    }
    event.preventDefault();
    const target = event.currentTarget;
    const { selectionStart, selectionEnd } = target;
    const next = `${value.slice(0, selectionStart)}  ${value.slice(selectionEnd)}`;
    onChange(next);
    requestAnimationFrame(() => {
      target.selectionStart = selectionStart + 2;
      target.selectionEnd = selectionStart + 2;
    });
  };

  const jumpTo = (line: number): void => {
    const target = area.current;
    if (target === null) {
      return;
    }
    const start = value.split('\n').slice(0, line - 1).join('\n').length + (line > 1 ? 1 : 0);
    target.focus();
    target.setSelectionRange(start, start);
  };

  return (
    <div className="css-editor">
      <label htmlFor="theme-custom-css">{t('themeSetting.customCss')}</label>
      <p className="field-note">{t('themeSetting.customCssHelp')}</p>
      <div className="css-editor-frame">
        <pre className="css-editor-gutter" ref={gutter} aria-hidden="true">
          {Array.from({ length: lines }, (_, index) => (
            <span key={index} data-bad={badLines.has(index + 1)}>
              {index + 1}
            </span>
          ))}
        </pre>
        <textarea
          id="theme-custom-css"
          ref={area}
          value={value}
          spellCheck={false}
          wrap="off"
          rows={18}
          aria-invalid={errors.length > 0}
          aria-describedby={errors.length > 0 ? 'theme-css-errors' : undefined}
          onKeyDown={onKeyDown}
          onBlur={(event) => {
            delete event.currentTarget.dataset['escaped'];
          }}
          onScroll={(event) => {
            if (gutter.current !== null) {
              gutter.current.scrollTop = event.currentTarget.scrollTop;
            }
          }}
          onChange={(event) => {
            onChange(event.target.value);
          }}
        />
      </div>
      <p className="field-note css-editor-size" data-over={byteLength(value) > maxBytes}>
        {t('theme.cssSize', { used: byteLength(value), max: maxBytes })}
      </p>
      {errors.length > 0 ? (
        <div id="theme-css-errors" className="css-editor-errors" role="alert">
          <p>{t('theme.cssErrors')}</p>
          <ul>
            {errors.map((message) => {
              const line = lineOf(message);
              return (
                <li key={message}>
                  {line === null ? (
                    message
                  ) : (
                    <button
                      type="button"
                      onClick={() => {
                        jumpTo(line);
                      }}
                    >
                      {message.replace(/^Custom CSS: /, '')}
                    </button>
                  )}
                </li>
              );
            })}
          </ul>
        </div>
      ) : null}
    </div>
  );
};

/** What each generated custom property means, by the token it comes from. */
const meaningOf = (name: string): ThemeMessageKey => {
  const exact: Readonly<Record<string, ThemeMessageKey>> = {
    'color-brand': 'themeSetting.brandColorHelp',
    'color-on-brand': 'theme.onBrand',
    'color-accent': 'themeSetting.accentColorHelp',
    'color-background': 'themeSetting.backgroundColorHelp',
    'color-surface': 'themeSetting.surfaceColorHelp',
    'color-text': 'themeSetting.textColorHelp',
    'color-muted': 'themeSetting.mutedColorHelp',
    'color-link': 'themeSetting.linkColorHelp',
    'color-border': 'theme.varBorder',
    'font-heading': 'themeSetting.headingFontHelp',
    'font-body': 'themeSetting.bodyFontHelp',
    'font-size-base': 'themeSetting.baseFontSizeHelp',
    'line-height': 'theme.varLineHeight',
    radius: 'themeSetting.cornerRadiusHelp',
    'radius-sm': 'theme.varRadiusSm',
    'content-width': 'themeSetting.contentWidthHelp',
  };
  const found = exact[name];
  if (found !== undefined) {
    return found;
  }
  if (name.startsWith('font-size-')) {
    return 'theme.varFontSize';
  }
  if (name.startsWith('space-')) {
    return 'theme.varSpace';
  }
  if (name.startsWith('header-')) {
    return 'themeSetting.headerStyleHelp';
  }
  return name.startsWith('button-') ? 'themeSetting.buttonStyleHelp' : 'theme.variables';
};

/** The token `<style>` the frame writes before the theme sheet, from a preview document. */
const tokenCss = (html: string | null): string | null => {
  if (html === null) {
    return null;
  }
  const document = new DOMParser().parseFromString(html, 'text/html');
  const style = document.querySelector('head style:not(#custom-css)');
  return style === null ? null : style.textContent.trim();
};

/** Every `--name: value;` in the token CSS, the first occurrence of each. */
const variablesOf = (css: string): readonly (readonly [string, string])[] => {
  const seen = new Map<string, string>();
  for (const match of css.matchAll(/--([a-z0-9-]+):\s*([^;]+);/g)) {
    const [, name, value] = match;
    if (name !== undefined && value !== undefined && !seen.has(name)) {
      seen.set(name, value.trim());
    }
  }
  return [...seen.entries()];
};

const CopyButton = ({ text }: { readonly text: string }): ReactElement => {
  const t = useThemeText();
  const [copied, setCopied] = useState(false);
  return (
    <button
      type="button"
      onClick={() => {
        void navigator.clipboard.writeText(text).then(() => {
          setCopied(true);
        });
      }}
    >
      {copied ? t('theme.copied') : t('theme.copy')}
    </button>
  );
};

const ThemeCssViewer = ({ previewHtml }: { readonly previewHtml: string | null }): ReactElement => {
  const t = useThemeText();
  const [sheet, setSheet] = useState<string | null>(null);
  const tokens = useMemo(() => tokenCss(previewHtml), [previewHtml]);

  useEffect(() => {
    let cancelled = false;
    fetch(`${defaultBaseUrl()}/assets/css/style.css`).then(
      async (response) => {
        const text = response.ok ? await response.text() : null;
        if (!cancelled) {
          setSheet(text);
        }
      },
      () => {
        if (!cancelled) {
          setSheet(null);
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <section className="theme-css" aria-labelledby="theme-css-heading">
      <h3 id="theme-css-heading">{t('theme.themeCss')}</h3>
      <p className="field-note">{t('theme.themeCssHelp')}</p>
      {tokens === null ? (
        <p className="field-note">{t('theme.variablesWait')}</p>
      ) : (
        <>
          <div className="theme-css-bar">
            <h4>{t('theme.variables')}</h4>
            <CopyButton text={tokens} />
          </div>
          <div className="theme-variables-scroll">
            <table className="theme-variables">
              <thead>
                <tr>
                  <th scope="col">{t('theme.variable')}</th>
                  <th scope="col">{t('theme.value')}</th>
                  <th scope="col">{t('theme.meaning')}</th>
                </tr>
              </thead>
              <tbody>
                {variablesOf(tokens).map(([name, value]) => (
                  <tr key={name}>
                    <td>
                      <code>--{name}</code>
                    </td>
                    <td>
                      <code>{value}</code>
                    </td>
                    <td>{t(meaningOf(name))}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
      {sheet === null ? null : (
        <details>
          <summary>{t('theme.stylesheet')}</summary>
          <div className="theme-css-bar">
            <CopyButton text={sheet} />
          </div>
          <pre className="theme-css-sheet">{sheet}</pre>
        </details>
      )}
    </section>
  );
};

export const CustomCssTab = ({
  value,
  maxBytes,
  errors,
  previewHtml,
  onChange,
}: {
  readonly value: string;
  readonly maxBytes: number;
  readonly errors: readonly string[];
  readonly previewHtml: string | null;
  readonly onChange: (value: string) => void;
}): ReactElement => (
  <div className="theme-css-tab">
    <CssEditor value={value} maxBytes={maxBytes} errors={errors} onChange={onChange} />
    <ThemeCssViewer previewHtml={previewHtml} />
  </div>
);

export default CustomCssTab;
