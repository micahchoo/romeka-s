/**
 * A plain preview panel: the rendered public page in a frame, laid out at the
 * chosen device width and scaled down to the panel (`../frameFit.ts`). The
 * Theme page shows it; the page editor shows the live surface in `../frame/`.
 * The frame keeps its size while a render is in flight, so the page around it
 * never shifts, and the previous render stays visible until the new one
 * arrives.
 */
import type { ReactElement } from 'react';
import { useRef, useState } from 'react';
import { useTranslator } from '../../../../i18n';
import { DeviceBar } from '../DeviceBar';
import { fitFrame, frameStyle, useBoxSize } from '../frameFit';
import type { DeviceWidth } from '../frameFit';
import { sites } from '../../../../i18n/namespaces/sites';

export type PreviewStatus = 'idle' | 'loading' | 'ready' | 'unavailable' | 'error';

export interface PreviewPanelProps {
  readonly html: string | null;
  readonly status: PreviewStatus;
  readonly message: string | null;
}

export const PreviewPanel = ({ html, status, message }: PreviewPanelProps): ReactElement => {
  const { t } = useTranslator(sites);
  const [device, setDevice] = useState<DeviceWidth>('fit');
  const frameBox = useRef<HTMLDivElement | null>(null);
  const box = useBoxSize(frameBox);

  return (
    <section className="editor-preview" role="region" aria-label={t('editor.previewLabel')}>
      <div className="editor-preview-bar">
        <DeviceBar device={device} onChange={setDevice} />
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
            style={frameStyle(fitFrame(device, box))}
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
