/**
 * The add-media panel: one source box and its rows, inline under the item
 * page's media heading so the grid stays visible and a new card lands where
 * the curator looks.
 *
 * The rows live in `store.ts`, which outlives this component, so a panel closed
 * while nothing is sending does not lose a row, and a media created after it
 * closed still reaches the grid through `onAdded`.
 */
import { useEffect, useMemo, useState, useSyncExternalStore } from 'react';
import type { ReactElement } from 'react';
import { useTranslator } from '../../../i18n';
import type { Picked } from '../../../app/upload/pick';
import type { Item, Media } from '../../../api/types';
import { AddRows } from './rows';
import { SourceBox } from './source-box';
import { addsFor } from './store';
import { mediaAdd } from '../../../i18n/namespaces/media-add';
import './add-media.css';

const ADVANCED_KEY = 'romeka.addMedia.advanced';

const readAdvanced = (): boolean => {
  try {
    return globalThis.localStorage.getItem(ADVANCED_KEY) === '1';
  } catch {
    return false;
  }
};

const rememberAdvanced = (advanced: boolean): void => {
  try {
    globalThis.localStorage.setItem(ADVANCED_KEY, advanced ? '1' : '0');
  } catch {
    // A browser that stores nothing still shows the panel.
  }
};

export interface AddMediaPanelProps {
  readonly item: Item;
  /** One media now exists on the item: the grid takes it. */
  readonly onAdded: (media: Media) => void;
  readonly onClose: () => void;
  /** Files a window drop collected before the panel opened. */
  readonly incoming?: Picked | null;
  readonly onIncoming?: () => void;
}

const AddMediaPanel = ({ item, onAdded, onClose, incoming, onIncoming }: AddMediaPanelProps): ReactElement => {
  const { t } = useTranslator(mediaAdd);
  const store = useMemo(() => addsFor(item), [item]);
  const snapshot = useSyncExternalStore(store.subscribe, store.snapshot);
  const [advanced, setAdvanced] = useState(readAdvanced);
  const busy = store.busy();

  useEffect(() => store.subscribeAdded(onAdded), [store, onAdded]);

  useEffect(() => {
    if (incoming !== null && incoming !== undefined) {
      store.addPicked(incoming);
      onIncoming?.();
    }
  }, [incoming, store, onIncoming]);

  return (
    <section
      className="add-media"
      aria-label={t('add.panelLabel', { title: item.title ?? '' })}
      onKeyDown={(event) => {
        if (event.key === 'Escape' && !store.busy()) {
          event.stopPropagation();
          onClose();
        }
      }}
    >
      {busy ? <p className="add-hint">{t('add.closeBusy')}</p> : null}
      <SourceBox
        store={store}
        snapshot={snapshot}
        advanced={advanced}
        onAdvancedChange={(next) => {
          setAdvanced(next);
          rememberAdvanced(next);
        }}
      />
      <AddRows store={store} snapshot={snapshot} />
    </section>
  );
};

export default AddMediaPanel;
