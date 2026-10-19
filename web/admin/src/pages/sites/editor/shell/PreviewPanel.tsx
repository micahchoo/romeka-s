/**
 * The live preview panel: the rendered public page in a frame, laid out at one
 * of three device widths and scaled down to the panel (`../frameFit.ts`). The frame keeps its size while a render is in flight, so the page
 * around it never shifts, and the previous render stays visible until the new
 * one arrives.
 */
import type { ReactElement } from 'react';
import { useRef, useState } from 'react';
import { useTranslator } from '../../../../i18n';
import type { MessageKey } from '../../../../i18n';
import { fitFrame, frameStyle, useBoxSize } from '../frameFit';

export type PreviewStatus = 'idle' | 'loading' | 'ready' | 'unavailable' | 'error';

export interface PreviewPanelProps {
  readonly html: string | null;
  readonly status: PreviewStatus;
  readonly message: string | null;
}

/** The three widths, in CSS pixels. */
const WIDTHS: readonly { readonly key: MessageKey; readonly width: number }[] = [
  { key: 'editor.widthDesktop', width: 1280 },
  { key: 'editor.widthTablet', width: 768 },
  { key: 'editor.widthMobile', width: 390 },
];

export const PreviewPanel = ({ html, status, message }: PreviewPanelProps): ReactElement => {
  const { t } = useTranslator();
  const [width, setWidth] = useState(WIDTHS[0]?.width ?? 1280);
  const frameBox = useRef<HTMLDivElement | null>(null);
  const box = useBoxSize(frameBox);
  const fit = fitFrame(width, box);

  return (
    <section className="editor-preview" role="region" aria-label={t('editor.previewLabel')}>
      <div className="editor-preview-bar">
        <div className="editor-preview-widths" role="group" aria-label={t('editor.previewWidth')}>
          {WIDTHS.map((preset) => (
            <button
              key={preset.key}
              type="button"
              aria-pressed={width === preset.width}
              onClick={() => {
                setWidth(preset.width);
              }}
            >
              {t(preset.key)}
            </button>
          ))}
        </div>
        <span className="editor-preview-status" role="status">
          {status === 'loading' ? t('editor.previewLoading') : ''}
        </span>
      </div>
      <div className="editor-preview-frame" ref={frameBox}>
        {html === null ? (
          <p className="editor-preview-empty">{t('editor.previewLabel')}</p>
        ) : (
          <iframe
            sandbox="allow-same-origin"
            srcDoc={html}
            title={t('editor.previewFrame')}
            style={{
              ...frameStyle(fit),
              left: `${String(Math.max(0, (box.width - width) / 2))}px`,
            }}
          />
        )}
      </div>
      {status === 'unavailable' ? (
        <p className="field-note">{t('editor.previewUnavailable')}</p>
      ) : null}
      {status === 'error' && message !== null ? (
        <p className="field-error" role="alert">
          {t('editor.previewError', { message })}
        </p>
      ) : null}
    </section>
  );
};
