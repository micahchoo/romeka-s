/**
 * Where a filled list is sent: a drop zone that is also a button. Both ways
 * lead to one file. The input sits inside its label, so the label opens it
 * and keyboard focus lands on it. While `canSend` is false (the import is
 * not editable, a save is waiting, a list is on its way) it says why in
 * words rather than only going dim.
 */
import type { DragEvent, ReactElement } from 'react';
import { useListsText } from './messages';

export const ListDrop = ({
  canSend,
  waitingForSave,
  onFile,
}: {
  readonly canSend: boolean;
  /** An edit is not saved yet: a list sent now would be read under the older revision. */
  readonly waitingForSave: boolean;
  readonly onFile: (file: File) => void;
}): ReactElement => {
  const { t } = useListsText();
  const onDrop = (event: DragEvent<HTMLDivElement>): void => {
    event.preventDefault();
    const file = event.dataTransfer.files[0];
    if (file !== undefined && canSend) {
      onFile(file);
    }
  };
  return (
    <div
      className="import-drop"
      onDragOver={(event) => {
        event.preventDefault();
      }}
      onDrop={onDrop}
    >
      <p>{t('lists.trip.drop')}</p>
      <p className="muted">{t('lists.trip.kinds')}</p>
      <label className="button-link import-file">
        {t('lists.trip.choose')}
        <input
          type="file"
          accept=".csv,.tsv"
          className="import-file-input"
          disabled={!canSend}
          onChange={(event) => {
            const file = event.target.files?.[0];
            if (file !== undefined) {
              onFile(file);
            }
            event.target.value = '';
          }}
        />
      </label>
      {waitingForSave ? <p className="muted">{t('lists.trip.waitSave')}</p> : null}
    </div>
  );
};
