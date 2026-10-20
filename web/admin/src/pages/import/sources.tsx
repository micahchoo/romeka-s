/**
 * Step 4, Sources: the optional spreadsheet and how to read it, and the
 * metadata tags inside the files. Every choice is a plan edit; the server
 * reads the CSV, detects its shape, repairs it and joins it to the items, and
 * this step shows what it answered.
 */
import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import type { CsvPreview, PreviewRequest } from '../../api/import-preview';
import type { CsvPlan, ImportPlan } from '../../api/imports';
import { ErrorNotice } from '../../app/errors';
import { useImportText } from './messages';
import type { ImportMessageKey } from './messages';
import { DELIMITERS, asDelimiter, newCsv, withCsv } from './plan-changes';
import type { Delimiter } from './plan-changes';
import { PreviewError } from './preview-error';
import { TagsSection } from './sources-tags';
import type { TabProps } from './tabs';
import { requestKey, usePreview } from './use-preview';

type Change = TabProps['onChange'];

const DELIMITER_TEXT: Readonly<Record<Delimiter, ImportMessageKey>> = {
  ',': 'sources.comma',
  ';': 'sources.semicolon',
  '\t': 'sources.tab',
};

/** The header-row choices: the first lines, and whatever the plan or the server names beyond them. */
const HEADER_LINES = 5;

const RawLines = ({ csv, headerRow }: { readonly csv: CsvPreview; readonly headerRow: number }): ReactElement => {
  const { t } = useImportText();
  const width = String(csv.rawLines.length).length;
  return (
    <pre className="import-raw" aria-label={t('sources.rawLabel')}>
      {csv.rawLines.map((text, index) => {
        const number = index + 1;
        const prefix = `${String(number).padStart(width)}  `;
        return (
          <span key={number} className="import-raw-line">
            <span className="import-raw-number">{prefix}</span>
            {number === headerRow ? (
              <>
                <mark>{text}</mark> <small>{t('sources.headerMark')}</small>
              </>
            ) : (
              text
            )}
            {'\n'}
          </span>
        );
      })}
    </pre>
  );
};

interface SheetProps {
  readonly csv: CsvPreview;
  readonly sheet: CsvPlan;
  readonly editable: boolean;
  readonly onChange: Change;
}

const ReadingChoices = ({ csv, sheet, editable, onChange }: SheetProps): ReactElement => {
  const { t, number } = useImportText();
  const set = (change: Partial<CsvPlan>): void => {
    onChange((plan) => withCsv(plan, (current) => ({ ...current, ...change })));
  };
  const detected = (text: string, isDetected: boolean): string =>
    isDetected ? t('sources.detected', { name: text }) : text;
  const lines = [...new Set([
    ...Array.from({ length: Math.min(HEADER_LINES, csv.rawLines.length) }, (_entry, index) => index + 1),
    sheet.headerRow,
    csv.detected.headerRow,
  ])].sort((a, b) => a - b);
  return (
    <>
      <fieldset disabled={!editable}>
        <legend>{t('sources.delimiter')}</legend>
        {DELIMITERS.map((delimiter) => (
          <label key={delimiter} className="import-radio">
            <input
              type="radio"
              name="import-delimiter"
              checked={sheet.delimiter === delimiter}
              onChange={() => { set({ delimiter }); }}
            />{' '}
            {detected(t(DELIMITER_TEXT[delimiter]), csv.detected.delimiter === delimiter)}
          </label>
        ))}
      </fieldset>
      {csv.rowCountError === 'delimiter' ? <p role="alert">{t('error.delimiter')}</p> : null}
      <fieldset disabled={!editable}>
        <legend>{t('sources.headerRow')}</legend>
        {lines.map((line) => (
          <label key={line} className="import-radio">
            <input
              type="radio"
              name="import-header-row"
              checked={sheet.headerRow === line}
              onChange={() => { set({ headerRow: line }); }}
            />{' '}
            {detected(t('sources.line', { number: line }), csv.detected.headerRow === line)}
          </label>
        ))}
      </fieldset>
      {csv.rowCountError === 'headerRow' ? <p role="alert">{t('error.headerRow')}</p> : null}
      <fieldset disabled={!editable}>
        <legend>{t('sources.repairs')}</legend>
        <label className="import-radio">
          <input
            type="checkbox"
            checked={sheet.windows1252}
            onChange={(event) => { set({ windows1252: event.target.checked }); }}
          />{' '}
          {t('sources.windows1252', { count: number(csv.cellsLookLike1252) })}
        </label>
        <label className="import-radio">
          <input type="checkbox" checked={sheet.trim} onChange={(event) => { set({ trim: event.target.checked }); }} />{' '}
          {t('sources.trim', { count: number(csv.cellsWithSpaces) })}
        </label>
      </fieldset>
    </>
  );
};

const JoinChoice = ({ csv, sheet, editable, onChange }: SheetProps): ReactElement => {
  const { t, number } = useImportText();
  return (
    <>
      <p>
        <label>
          {t('sources.join')}{' '}
          <select
            value={sheet.joinColumn}
            disabled={!editable}
            onChange={(event) => {
              const joinColumn = event.target.value;
              onChange((plan) => withCsv(plan, (current) => ({ ...current, joinColumn })));
            }}
          >
            <option value="">{t('sources.joinChoose')}</option>
            {csv.head.map((header) => (
              <option key={header} value={header}>
                {header}
              </option>
            ))}
          </select>
        </label>{' '}
        <span className="query-count">{t('sources.joinHelp')}</span>
      </p>
      {sheet.joinColumn === '' ? null : (
        <p className="query-count">
          {t('sources.joined', {
            joined: number(csv.joined),
            rows: number(csv.rowsWithoutFile),
            items: number(csv.itemsWithoutRow),
          })}
        </p>
      )}
    </>
  );
};

const SheetChoice = ({
  files,
  plan,
  editable,
  onChoose,
}: {
  readonly files: readonly string[];
  readonly plan: ImportPlan;
  readonly editable: boolean;
  readonly onChoose: (file: string | null) => void;
}): ReactElement => {
  const { t } = useImportText();
  return (
    <fieldset disabled={!editable}>
      <legend>{t('sources.sheet')}</legend>
      <label className="import-radio">
        <input type="radio" name="import-sheet" checked={plan.csv === null} onChange={() => { onChoose(null); }} />{' '}
        {t('sources.none')}
      </label>
      {files.map((file) => (
        <label key={file} className="import-radio">
          <input
            type="radio"
            name="import-sheet"
            checked={plan.csv?.file === file}
            onChange={() => { onChoose(file); }}
          />{' '}
          <span className="import-mono">{file}</span>
        </label>
      ))}
      {files.length === 0 ? <p className="query-count">{t('sources.noCsvFiles')}</p> : null}
    </fieldset>
  );
};

/** The preview this step reads: the CSV, the tags, and counts only of files and rows. */
const sourcesRequest = (plan: ImportPlan): PreviewRequest => ({
  include: ['csv', 'tags', 'files', 'rows'],
  files: { filter: 'match', offset: 0, limit: 0 },
  rows: { problemsOnly: false, offset: 0, limit: 0 },
  plan,
});

export const SourcesStep = ({ detail, plan, editable, onChange }: TabProps): ReactElement => {
  const { t } = useImportText();
  const request = sourcesRequest(plan);
  const state = usePreview(detail.id, request);
  // A newly chosen spreadsheet takes the delimiter and header row the server detects, once.
  const [adopt, setAdopt] = useState<string | null>(null);
  const preview = state.preview;
  const fresh = state.answers === requestKey(request);
  const csv = preview?.csv;
  useEffect(() => {
    if (adopt === null || !fresh || csv === undefined || plan.csv?.file !== adopt || csv.rawLines.length === 0) {
      return;
    }
    setAdopt(null);
    const delimiter = asDelimiter(csv.detected.delimiter);
    onChange((current) =>
      withCsv(current, (sheet) => ({ ...sheet, delimiter: delimiter ?? sheet.delimiter, headerRow: csv.detected.headerRow })),
    );
  }, [adopt, fresh, csv, plan.csv?.file, onChange]);
  const choose = (file: string | null): void => {
    setAdopt(file);
    onChange((current) => ({ ...current, csv: file === null ? null : newCsv(file) }));
  };
  const rowCountKey = csv?.rowCountError ?? null;
  return (
    <section className="import-sources">
      {state.error === null ? null : <ErrorNotice error={state.error} />}
      {preview?.error?.key === rowCountKey ? null : <PreviewError preview={preview} />}
      <h3>{t('sources.sheetHeading')}</h3>
      {csv === undefined ? (
        <p role="status">{t('archive.loadingPreview')}</p>
      ) : (
        <>
          <SheetChoice files={csv.files} plan={plan} editable={editable} onChoose={choose} />
          {plan.csv === null ? (
            <p className="query-count">{t('sources.noSheet')}</p>
          ) : (
            <div aria-busy={state.loading}>
              <RawLines csv={csv} headerRow={plan.csv.headerRow} />
              <ReadingChoices csv={csv} sheet={plan.csv} editable={editable} onChange={onChange} />
              <JoinChoice csv={csv} sheet={plan.csv} editable={editable} onChange={onChange} />
            </div>
          )}
        </>
      )}
      <TagsSection
        plan={plan}
        tags={preview?.tags}
        matched={preview?.files?.matched ?? 0}
        columns={preview?.rows?.columns ?? []}
        editable={editable}
        onChange={onChange}
      />
    </section>
  );
};
