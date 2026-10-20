/**
 * The grid's two panels: Fill, which writes one value into every empty cell
 * of a field, and Map…, which maps a spreadsheet column the plan leaves out
 * to a field the grid has, a field from the vocabularies, or a new local
 * field whose term the server names.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import type { RowsPreview } from '../../api/import-preview';
import type { CsvPlan, ImportId, ImportPlan } from '../../api/imports';
import { ErrorNotice } from '../../app/errors';
import { useAsync } from '../../app/useAsync';
import { useTermLabels } from '../../i18n/termText';
import { collectRows } from './collect-rows';
import { fieldOptions, loadProperties } from './field-options';
import { useImportText } from './messages';
import { fillFor, withEdits } from './plan-changes';
import type { TabProps } from './tabs';

/** The token a fill value names each row's identifier with. */
const TOKEN = '{identifier}';

/** Which rows hold an empty cell of `term`: the ones the server flags, when it flags them. */
const emptyCells = async (
  id: ImportId,
  plan: ImportPlan,
  term: string,
  flagged: boolean,
): Promise<readonly string[]> => {
  const { rows } = await collectRows(id, plan, flagged ? { kind: 'empty', term } : {});
  return rows
    .filter((row) => !row.skipped && (row.cells.find((cell) => cell.term === term)?.value.trim() ?? '') === '')
    .map((row) => row.identifier);
};

export const FillPanel = ({
  id,
  plan,
  term,
  label,
  flagged,
  onChange,
  onClose,
}: {
  readonly id: ImportId;
  readonly plan: ImportPlan;
  readonly term: string;
  readonly label: string;
  /** Whether the server counts this field's empty cells as a problem kind. */
  readonly flagged: boolean;
  readonly onChange: TabProps['onChange'];
  readonly onClose: () => void;
}): ReactElement => {
  const { t, number } = useImportText();
  const [text, setText] = useState('');
  // Counted once per panel: the cells empty when it opened are the ones it fills.
  const empty = useAsync(() => emptyCells(id, plan, term, flagged), [id, term, flagged]);
  const identifiers = empty.status === 'ready' ? empty.data : [];
  return (
    <form
      className="import-panel"
      onSubmit={(event) => {
        event.preventDefault();
        if (text === '' || identifiers.length === 0) {
          return;
        }
        onChange((current) => withEdits(current, term, new Map(identifiers.map((row) => [row, fillFor(text, row)]))));
        onClose();
      }}
    >
      <h3>{t('fill.heading', { field: label })}</h3>
      <p>
        <label>
          {t('fill.value', { token: TOKEN })}
          <input
            className="import-wide"
            value={text}
            placeholder={t('fill.placeholder', { token: TOKEN })}
            onChange={(event) => { setText(event.target.value); }}
          />
        </label>
      </p>
      {empty.status === 'error' ? <ErrorNotice error={empty.error} /> : null}
      <div className="import-chips">
        <button type="submit" className="button-primary" disabled={text === '' || identifiers.length === 0}>
          {empty.status === 'loading' ? t('fill.counting') : t('fill.submit', { count: number(identifiers.length) })}
        </button>
        <button type="button" onClick={onClose}>
          {t('fill.cancel')}
        </button>
      </div>
    </form>
  );
};

const StandardField = ({
  columns,
  onPick,
}: {
  readonly columns: RowsPreview['columns'];
  readonly onPick: (term: string) => void;
}): ReactElement => {
  const { t } = useImportText();
  const labels = useTermLabels();
  const properties = useAsync(loadProperties, []);
  const [term, setTerm] = useState('');
  if (properties.status === 'loading') {
    return <p role="status">{t('map.loading')}</p>;
  }
  if (properties.status === 'error') {
    return <ErrorNotice error={properties.error} />;
  }
  const taken = new Set(columns.map((column) => column.term));
  const options = fieldOptions(properties.data, [], labels).filter((option) => !taken.has(option.term));
  return (
    <p>
      <label>
        {t('map.standard')}{' '}
        <select value={term} onChange={(event) => { setTerm(event.target.value); }}>
          <option value="">{t('map.choose')}</option>
          {options.map((option) => (
            <option key={option.term} value={option.term}>
              {t('tags.fieldOption', { label: option.label, term: option.term })}
            </option>
          ))}
        </select>
      </label>{' '}
      <button type="button" disabled={term === ''} onClick={() => { onPick(term); }}>
        {t('map.add')}
      </button>
    </p>
  );
};

export const MapPanel = ({
  header,
  columns,
  label,
  onMap,
  onClose,
}: {
  readonly header: string;
  readonly columns: RowsPreview['columns'];
  readonly label: (term: string) => string;
  readonly onMap: (target: CsvPlan['columns'][string]) => void;
  readonly onClose: () => void;
}): ReactElement => {
  const { t } = useImportText();
  const [name, setName] = useState(header);
  return (
    <form
      className="import-panel"
      onSubmit={(event) => {
        event.preventDefault();
        if (name.trim() !== '') {
          onMap({ newField: name.trim() });
        }
      }}
    >
      <h3>{t('map.heading', { header })}</h3>
      <p>{t('map.existing')}</p>
      <div className="import-chips">
        {columns.map((column) => (
          <button key={column.term} type="button" onClick={() => { onMap(column.term); }}>
            {label(column.term)}
          </button>
        ))}
      </div>
      <StandardField columns={columns} onPick={onMap} />
      <p>
        <label>
          {t('map.newLabel')}
          <input className="import-wide" value={name} onChange={(event) => { setName(event.target.value); }} />
        </label>
      </p>
      <p className="query-count">{t('map.newNote')}</p>
      <div className="import-chips">
        <button type="submit" className="button-primary" disabled={name.trim() === ''}>
          {t('map.create')}
        </button>
        <button type="button" onClick={onClose}>
          {t('map.cancel')}
        </button>
      </div>
    </form>
  );
};
