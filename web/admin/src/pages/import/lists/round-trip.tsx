/**
 * "Fill in a list yourself" (`docs/import-v2.md`, "The list round trip"):
 * choose the columns, see the first rows exactly as they download, download
 * the list, fill it in, upload it, and read what the upload did: the rows it
 * read and the rows that belong to no item of this import.
 */
import { useId, useState } from 'react';
import type { ReactElement } from 'react';
import { importApi } from '../../../api/import-routes';
import type { ImportPlan, SlotRule, SlotRules } from '../../../api/import-plan';
import type { ImportDetail } from '../../../api/import-view';
import { ErrorNotice } from '../../../app/errors';
import { TableScroll } from '../../../app/table-scroll';
import { toError, useAsync } from '../../../app/useAsync';
import { useDraft } from '../draft-context';
import { useListsText } from './messages';
import { loadProperties } from './catalog';
import { csvHead } from './csv-head';
import { ListDrop } from './list-drop';

/** The columns the server takes besides properties (`template.rs`, `ListColumn`). */
const SLOT_COLUMNS = ['sets', 'template', 'visibility'] as const;
const FIRST_COLUMNS: readonly string[] = ['dcterms:title', 'dcterms:date'];
const ROWS_SHOWN = 3;
const STRAYS_SHOWN = 50;

export interface UploadResult {
  readonly sheet: string;
  readonly file: string;
  readonly rows: number;
  /** Rows whose Identifier is no item of this import: their record has no file. */
  readonly strays: number;
  readonly strayKeys: readonly string[];
  readonly mapped: readonly string[];
  readonly unmapped: readonly string[];
}

const withoutSheet = (rules: SlotRules, sheet: string): SlotRules => {
  const next: { -readonly [slot in keyof SlotRules]: SlotRules[slot] } = {};
  for (const [slot, rule] of Object.entries<SlotRule | undefined>(rules)) {
    if (rule !== undefined) {
      Object.assign(next, {
        [slot]: { ...rule, sources: rule.sources.filter((source) => !(source.from === 'column' && source.sheet === sheet)) },
      });
    }
  }
  return next;
};

/** The plan without a list: its sheet and every field source that reads it. */
export const removeSheet = (plan: ImportPlan, sheet: string): ImportPlan => ({
  ...plan,
  sheets: plan.sheets.filter((one) => one.id !== sheet),
  slots: { item: withoutSheet(plan.slots.item, sheet), media: withoutSheet(plan.slots.media, sheet), itemSet: withoutSheet(plan.slots.itemSet, sheet) },
});

/** What an added list did, counted by the server. */
export const readUpload = async (before: ImportPlan, after: ImportDetail): Promise<UploadResult> => {
  const added = after.plan.sheets.find((sheet) => !before.sheets.some((old) => old.id === sheet.id));
  if (added === undefined) {
    throw new Error('The import has no new list.');
  }
  const preview = await importApi.preview(after.id, ['sheets']);
  const read = preview.sheets.find((sheet) => sheet.id === added.id)?.read ?? null;
  const strays = (
    await importApi.preview(after.id, ['entries'], { entries: { sheet: added.id, paired: false, perPage: STRAYS_SHOWN } })
  ).entries;
  const keys = strays.items.flatMap((entry) => ('row' in entry.entry ? [entry.entry.row.key] : []));
  const mapped = added.columns.map((column) => column.column.header);
  const kept = new Set(['Identifier', 'Files', ...mapped]);
  return {
    sheet: added.id,
    file: added.file,
    rows: read?.rows ?? 0,
    strays: strays.total,
    strayKeys: keys,
    mapped,
    unmapped: (read?.columns ?? []).map((column) => column.header).filter((header) => !kept.has(header)),
  };
};

type Upload =
  | { readonly at: 'idle' }
  | { readonly at: 'sending'; readonly name: string }
  | { readonly at: 'failed'; readonly error: Error }
  | { readonly at: 'read'; readonly result: UploadResult };

const Result = ({ result, onRemove }: { readonly result: UploadResult; readonly onRemove: () => void }): ReactElement => {
  const { t, number } = useListsText();
  const quote = (words: readonly string[]): string => words.map((word) => `“${word}”`).join(', ');
  return (
    <div className="import-note" role="status">
      <p>
        <b>{t('lists.trip.read', { count: number(result.rows), file: result.file })}</b>{' '}
        {t('lists.trip.belong', { count: number(result.rows - result.strays) })}{' '}
        {result.strays === 0 ? null : t('lists.trip.strays', { count: number(result.strays) })}
      </p>
      <p>{result.mapped.length === 0 ? t('lists.trip.fillsNone') : t('lists.trip.fills', { columns: quote(result.mapped) })}</p>
      {result.unmapped.length === 0 ? null : <p>{t('lists.trip.unused', { columns: quote(result.unmapped) })}</p>}
      {result.strays === 0 ? null : (
        <details>
          <summary>{t('lists.trip.showStrays', { count: number(result.strays) })}</summary>
          <p className="muted">{t('lists.trip.straysWhy')}</p>
          <ul>
            {result.strayKeys.map((key) => (
              <li key={key}>{key}</li>
            ))}
          </ul>
          {result.strays > result.strayKeys.length ? <p className="muted">{t('lists.trip.andMore', { count: number(result.strays - result.strayKeys.length) })}</p> : null}
        </details>
      )}
      <button type="button" onClick={onRemove}>
        {t('lists.trip.remove')}
      </button>
    </div>
  );
};

export const RoundTrip = (): ReactElement => {
  const { t, number } = useListsText();
  const draft = useDraft();
  const id = useId();
  const [columns, setColumns] = useState<readonly string[]>(FIRST_COLUMNS);
  const [upload, setUpload] = useState<Upload>({ at: 'idle' });
  const properties = useAsync(loadProperties, []);
  const head = useAsync(() => importApi.templateCsv(draft.id, columns).then((text) => csvHead(text, ROWS_SHOWN + 1)), [draft.id, draft.detail.revision, columns.join(',')]);
  const known = properties.status === 'ready' ? properties.data : [];
  const slotWords: Readonly<Record<(typeof SLOT_COLUMNS)[number], string>> = {
    sets: t('lists.trip.colSets'),
    template: t('lists.trip.colType'),
    visibility: t('lists.trip.colWho'),
  };
  const labelOf = (column: string): string => {
    const slot = SLOT_COLUMNS.find((one) => one === column);
    return slot === undefined ? (known.find((property) => property.term === column)?.label ?? column) : slotWords[slot];
  };
  // An edit not yet saved would be lost under the revision the upload makes.
  const canSend = draft.editable && draft.save.status === 'saved' && upload.at !== 'sending';

  const send = (file: File): void => {
    const before = draft.plan;
    setUpload({ at: 'sending', name: file.name });
    importApi
      .addList(draft.id, file)
      .then(async (detail) => {
        draft.adopt(detail);
        return readUpload(before, detail);
      })
      .then(
        (result) => {
          setUpload({ at: 'read', result });
        },
        (error: unknown) => {
          setUpload({ at: 'failed', error: toError(error) });
        },
      );
  };
  return (
    <section className="import-card import-trip" aria-labelledby={`${id}-title`}>
      <h4 id={`${id}-title`}>{t('lists.trip.title')}</h4>
      <p className="muted">{t('lists.trip.lede')}</p>
      <h5>{t('lists.trip.step1')}</h5>
      <div className="import-row-actions" role="group" aria-label={t('lists.trip.columns')}>
        {columns.map((column) => (
          <span key={column} className="import-chip">
            {labelOf(column)}{' '}
            <button
              type="button"
              className="link-button"
              aria-label={t('lists.trip.removeColumn', { column: labelOf(column) })}
              onClick={() => {
                setColumns(columns.filter((one) => one !== column));
              }}
            >
              ×
            </button>
          </span>
        ))}
        <select
          aria-label={t('lists.trip.addColumn')}
          value=""
          onChange={(event) => {
            if (event.target.value !== '') {
              setColumns([...columns, event.target.value]);
            }
          }}
        >
          <option value="">{t('lists.trip.addColumn')}</option>
          <optgroup label={t('lists.trip.omeka')}>
            {SLOT_COLUMNS.filter((slot) => !columns.includes(slot)).map((slot) => (
              <option key={slot} value={slot}>
                {slotWords[slot]}
              </option>
            ))}
          </optgroup>
          <optgroup label={t('lists.trip.fields')}>
            {known
              .filter((property) => !columns.includes(property.term))
              .map((property) => (
                <option key={property.term} value={property.term}>
                  {property.label ?? property.term}
                </option>
              ))}
          </optgroup>
        </select>
      </div>
      <p>{t('lists.trip.first')}</p>
      {head.status === 'error' ? <ErrorNotice error={head.error} /> : null}
      {head.status === 'ready' ? (
        <TableScroll label={t('lists.trip.firstTable')}>
          <table>
            <thead>
              <tr>
                {(head.data.rows[0] ?? []).map((cell, index) => (
                  <th key={index} scope="col">
                    {cell}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {head.data.rows.slice(1).map((row, line) => (
                <tr key={line}>
                  {row.map((cell, index) => (
                    <td key={index}>{cell}</td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </TableScroll>
      ) : null}
      <p className="import-note">{t('lists.trip.keep')}</p>
      <p>
        <a className="button-link" href={importApi.templateUrl(draft.id, columns)} download>
          {head.status === 'ready'
            ? t('lists.trip.download', { count: number(Math.max(0, head.data.count - 1)) })
            : t('lists.trip.downloadPlain')}
        </a>
      </p>
      <h5>{t('lists.trip.step2')}</h5>
      <ListDrop canSend={canSend} waitingForSave={draft.editable && draft.save.status !== 'saved'} onFile={send} />
      {upload.at === 'sending' ? <p role="status">{t('lists.trip.sending', { file: upload.name })}</p> : null}
      {upload.at === 'failed' ? <ErrorNotice error={upload.error} /> : null}
      {upload.at === 'read' ? (
        <Result
          result={upload.result}
          onRemove={() => {
            const sheet = upload.result.sheet;
            draft.change((plan) => removeSheet(plan, sheet));
            setUpload({ at: 'idle' });
          }}
        />
      ) : null}
    </section>
  );
};

export default RoundTrip;
