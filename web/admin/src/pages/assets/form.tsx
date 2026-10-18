/**
 * `Admin/AssetController::editAction` over the two members
 * `AssetAdapter::hydrate` writes: `o:name` and `o:alt_text`. The file identity
 * columns (`o:media_type`, `o:storage_id`, `o:extension`) are fixed when the
 * upload stores the file and no request changes them.
 */
import type { ReactElement, SyntheticEvent } from 'react';
import { useState } from 'react';
import { api } from '../../api/client';
import type { Asset, AssetWrite } from '../../api/types';
import { SaveStatus, SubmitRow, TextField } from '../../app/fields';
import { useSave } from '../../app/save';
import { useTranslator } from '../../i18n';

const nullable = (text: string): string | null => (text.trim() === '' ? null : text);

export const AssetEditForm = ({
  asset,
  onSaved,
}: {
  readonly asset: Asset;
  readonly onSaved: (asset: Asset) => void;
}): ReactElement => {
  const { t } = useTranslator();
  const [name, setName] = useState(asset.name);
  const [altText, setAltText] = useState(asset.altText ?? '');
  const [formError, setFormError] = useState<string | null>(null);
  const { state: saveState, save } = useSave();

  const submit = (event: SyntheticEvent<HTMLFormElement>): void => {
    event.preventDefault();
    if (name.trim() === '') {
      setFormError(t('assets.errorEmptyName'));
      return;
    }
    setFormError(null);
    const write: AssetWrite = { name, altText: nullable(altText) };
    save(async () => {
      onSaved(await api.updateAsset(asset.id, write));
    });
  };

  return (
    <form onSubmit={submit}>
      <fieldset>
        <legend>{t('assets.legend')}</legend>
        <TextField id="asset-name" label={t('column.name')} value={name} onChange={setName} />
        <p>
          <label htmlFor="asset-alt-text">{t('column.altText')}</label>
          <textarea
            id="asset-alt-text"
            rows={2}
            value={altText}
            onChange={(event) => {
              setAltText(event.target.value);
            }}
          />
        </p>
      </fieldset>
      {formError === null ? null : <p role="alert">{formError}</p>}
      <SaveStatus state={saveState} />
      <SubmitRow label={t('action.save')} busy={saveState.status === 'saving'} />
    </form>
  );
};
