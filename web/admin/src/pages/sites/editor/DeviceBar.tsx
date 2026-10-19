/**
 * The device choices above a preview: Fit, desktop, tablet and mobile. Both
 * previews (the page editor's and the Theme page's) show the same bar.
 */
import type { ReactElement } from 'react';
import { useTranslator } from '../../../i18n';
import { DEVICES } from './frameFit';
import type { DeviceWidth } from './frameFit';
import { sites } from '../../../i18n/namespaces/sites';

export const DeviceBar = ({
  device,
  onChange,
}: {
  readonly device: DeviceWidth;
  readonly onChange: (device: DeviceWidth) => void;
}): ReactElement => {
  const { t } = useTranslator(sites);
  return (
    <div className="editor-preview-widths" role="group" aria-label={t('editor.previewWidth')}>
      {DEVICES.map((preset) => (
        <button
          key={preset.key}
          type="button"
          aria-pressed={device === preset.width}
          onClick={() => {
            onChange(preset.width);
          }}
        >
          {t(preset.key)}
        </button>
      ))}
    </div>
  );
};
