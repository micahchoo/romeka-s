/**
 * The insert picker: a searchable list of the registered layouts. Choosing one
 * closes the picker and hands the layout name and the insertion index back to
 * the canvas, which dispatches `addBlock`.
 */
import { useState } from 'react';
import type { KeyboardEvent as ReactKeyboardEvent, ReactElement } from 'react';
import { useTranslator } from '../../../../i18n';
import type { BlockRegistry } from '../../blockSchema';

export interface BlockPickerProps {
  readonly registry: BlockRegistry;
  /** The flat index the chosen layout is inserted at. */
  readonly index: number;
  readonly onChoose: (layout: string, index: number) => void;
  readonly onClose: () => void;
}

const matches = (label: string, name: string, query: string): boolean => {
  const needle = query.trim().toLowerCase();
  return (
    needle === '' ||
    label.toLowerCase().includes(needle) ||
    name.toLowerCase().includes(needle)
  );
};

export const BlockPicker = ({
  registry,
  index,
  onChoose,
  onClose,
}: BlockPickerProps): ReactElement => {
  const { t } = useTranslator();
  const [query, setQuery] = useState('');
  const options = [...registry.layouts, ...registry.romeka].filter((layout) =>
    matches(t(layout.label), layout.name, query),
  );
  const searchId = `canvas-picker-search-${String(index)}`;

  const onKeyDown = (event: ReactKeyboardEvent<HTMLInputElement>): void => {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
    }
  };

  return (
    <div className="canvas-picker" role="dialog" aria-label={t('canvas.picker')}>
      <p>
        <label htmlFor={searchId}>{t('canvas.search')}</label>
        <input
          id={searchId}
          type="search"
          value={query}
          autoFocus
          onChange={(event) => {
            setQuery(event.target.value);
          }}
          onKeyDown={onKeyDown}
        />
      </p>
      {options.length === 0 ? (
        <p className="canvas-picker-empty">{t('canvas.noMatches')}</p>
      ) : (
        <ul className="canvas-picker-list">
          {options.map((layout) => (
            <li key={layout.name}>
              <button
                type="button"
                onClick={() => {
                  onChoose(layout.name, index);
                }}
              >
                {t(layout.label)}
              </button>
            </li>
          ))}
        </ul>
      )}
      <p>
        <button
          type="button"
          onClick={() => {
            onClose();
          }}
        >
          {t('canvas.close')}
        </button>
      </p>
    </div>
  );
};
