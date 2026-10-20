/**
 * The edit bar over the grid: the selected cell's value to change, its
 * problems in words, the choice between the spreadsheet's and the files'
 * values where they disagree, and Undo my edit for a cell the user changed.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import type { RowCell } from '../../api/import-preview';
import { useImportText } from './messages';
import { CELL_PROBLEM, cellSource } from './problem-text';

export interface EditBarProps {
  readonly identifier: string;
  readonly field: string;
  readonly cell: RowCell;
  readonly editable: boolean;
  readonly onSave: (value: string) => void;
  readonly onUndo: () => void;
  readonly onClose: () => void;
}

export const EditBar = ({ identifier, field, cell, editable, onSave, onUndo, onClose }: EditBarProps): ReactElement => {
  const translator = useImportText();
  const { t } = translator;
  const [value, setValue] = useState(cell.value);
  const problems = cell.problems.flatMap((kind) => {
    const key = CELL_PROBLEM[kind];
    return key === undefined ? [] : [t(key)];
  });
  const source = cellSource(translator, cell.source);
  const { conflict } = cell;
  return (
    <form
      className="import-panel import-edit"
      onSubmit={(event) => {
        event.preventDefault();
        onSave(value);
      }}
    >
      <div className="import-edit-bar">
        <label>
          {t('edit.label', { identifier, field })}
          <input
            className="import-wide"
            value={value}
            disabled={!editable}
            autoFocus
            onChange={(event) => { setValue(event.target.value); }}
          />
        </label>
        <button type="submit" className="button-primary" disabled={!editable}>
          {t('edit.save')}
        </button>
        <button type="button" onClick={onClose}>
          {t('edit.cancel')}
        </button>
        {cell.source === 'edit' ? (
          <button type="button" className="import-link" disabled={!editable} onClick={onUndo}>
            {t('edit.undo')}
          </button>
        ) : null}
      </div>
      {source === '' ? null : <p className="query-count">{source}</p>}
      {problems.length === 0 ? null : <p role="alert">{problems.join(' · ')}</p>}
      {conflict === undefined ? null : (
        <p>
          {t('edit.conflict', { csv: conflict.csv, files: conflict.files })}{' '}
          <button
            type="button"
            className="import-link"
            disabled={!editable}
            onClick={() => { onSave(conflict.csv); }}
          >
            {t('edit.useSheet')}
          </button>{' '}
          <button
            type="button"
            className="import-link"
            disabled={!editable}
            onClick={() => { onSave(conflict.files); }}
          >
            {t('edit.useFiles')}
          </button>
        </p>
      )}
    </form>
  );
};
