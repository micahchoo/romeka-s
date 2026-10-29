/**
 * The preview: how many items change, how many already match or may not be changed, and five
 * items before and after in words, their changed fields' values included. Apply starts the change on exactly these items.
 */
import type { ReactElement } from 'react';
import type { BulkPreview } from '../../../api/bulk';
import { useTranslator } from '../../../i18n';
import { bulk } from '../../../i18n/namespaces/bulk';
import { useTermLabels } from '../../../i18n/termText';
import { useClassNamesOf } from '../browse-options';
import type { BrowseOptions } from '../browse-options';
import { fieldValueWords, fieldWords } from './words';

const Lines = ({ lines }: { readonly lines: readonly string[] }): ReactElement => (
  <ul className="bulk-lines">
    {lines.map((line) => (
      <li key={line}>{line}</li>
    ))}
  </ul>
);

export const PreviewStep = ({
  preview,
  options,
  expired,
  busy,
  onApply,
  onPreviewAgain,
  onBack,
}: {
  readonly preview: BulkPreview;
  readonly options: BrowseOptions;
  /** Apply was refused: the preview is over ten minutes old. */
  readonly expired: boolean;
  readonly busy: boolean;
  readonly onApply: () => void;
  readonly onPreviewAgain: () => void;
  readonly onBack: () => void;
}): ReactElement => {
  const { t, number } = useTranslator(bulk);
  const text = useTermLabels();
  const classNames = useClassNamesOf(options.classes);
  const changing = preview.total - preview.unchanged - preview.denied;
  return (
    <div className="bulk-preview">
      <p className="bulk-headline" role="status">
        {changing === 0
          ? t('bulk.preview.changeNone')
          : changing === 1
            ? t('bulk.preview.changeOne')
            : t('bulk.preview.changeMany', { count: number(changing) })}
      </p>
      {preview.unchanged === 1 ? <p>{t('bulk.preview.unchangedOne')}</p> : null}
      {preview.unchanged > 1 ? <p>{t('bulk.preview.unchanged', { count: number(preview.unchanged) })}</p> : null}
      {preview.denied > 0 ? <p>{t('bulk.preview.denied', { count: number(preview.denied) })}</p> : null}
      {preview.samples.length === 0 ? null : (
        <table className="bulk-samples">
          <caption>{t('bulk.preview.samples')}</caption>
          <thead>
            <tr>
              <th scope="col">{t('bulk.preview.item')}</th>
              <th scope="col">{t('bulk.preview.before')}</th>
              <th scope="col">{t('bulk.preview.after')}</th>
            </tr>
          </thead>
          <tbody>
            {preview.samples.map((sample) => {
              const words = fieldWords({ t, options, classNames }, sample.before, sample.after);
              const fields = sample.values.map((change) => ({
                name: text.label(change.term, change.label),
                change,
              }));
              const before = [...words.before, ...fields.map(({ name, change }) => fieldValueWords(t, name, change.before))];
              const after = [...words.after, ...fields.map(({ name, change }) => fieldValueWords(t, name, change.after))];
              return (
                <tr key={String(sample.id)}>
                  <th scope="row">
                    <a href={`#/items/${String(sample.id)}`}>
                      {sample.title ?? t('bulk.word.untitled', { id: String(sample.id) })}
                    </a>
                  </th>
                  <td>
                    <Lines lines={before} />
                  </td>
                  <td>
                    <Lines lines={after} />
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
      {expired ? (
        <p role="alert">{t('bulk.preview.expired')}</p>
      ) : null}
      <p className="bulk-actions">
        {expired ? (
          <button type="button" disabled={busy} onClick={onPreviewAgain}>
            {t('bulk.preview.again')}
          </button>
        ) : (
          <button type="button" disabled={busy || changing === 0} aria-busy={busy} onClick={onApply}>
            {t('bulk.preview.apply')}
          </button>
        )}
        <button type="button" className="link-button" disabled={busy} onClick={onBack}>
          {t('bulk.preview.back')}
        </button>
      </p>
    </div>
  );
};
