/**
 * Step 1, Folder: how file names become items. "Mark an example" compiles the
 * marks to the plan's pattern; "Write a regex" edits the pattern itself. The
 * server matches every file and answers a page of them at a time.
 */
import { useEffect, useState } from 'react';
import type { ReactElement } from 'react';
import type { PreviewRequest, FilesQuery } from '../../api/import-preview';
import type { ImportExample, ImportPlan, Label } from '../../api/imports';
import {
  compileExample,
  defaultExample,
  optionalCandidates,
  optionalInUse,
  splitExample,
  withOptional,
} from './example';
import { ExampleMarker, ExampleShape } from './example-marker';
import { FileTable, filesQuery } from './files';
import type { FileView } from './files';
import { useImportText } from './messages';
import { PreviewError } from './preview-error';
import type { ImportMessageKey } from './messages';
import { UndoBanner } from './undo';
import type { TabProps } from './tabs';
import { requestKey, usePreview } from './use-preview';

type Mode = 'example' | 'regex';

const PRESETS = [
  {
    shape: '{set}/…/{identifier}.{ext}',
    pattern: '^(?<set>[^/]+)/(?:[^/]+/)*(?<identifier>[^/]+)\\.(?<ext>[A-Za-z0-9]+)$',
  },
  {
    shape: '{set}/{identifier}/{page}.{ext}',
    pattern: '^(?<set>[^/]+)/(?<identifier>[^/]+)/(?<page>[0-9]+)\\.(?<ext>[A-Za-z0-9]+)$',
  },
] as const;

const NO_PART: Readonly<Partial<Record<Label, { readonly missing: ImportMessageKey; readonly optional: ImportMessageKey }>>> = {
  part: { missing: 'example.noPart', optional: 'example.partOptional' },
  page: { missing: 'example.noPage', optional: 'example.pageOptional' },
};

/** The example with the same marks moved to another file: marks on parts it does not have drop. */
const moveExample = (example: ImportExample, path: string): ImportExample => {
  const parts = splitExample(path);
  const keys = new Set([...parts.folders.map((_folder, index) => `d${String(index)}`), ...parts.name.map((part) => part.key)]);
  return {
    path,
    labels: Object.fromEntries(Object.entries(example.labels).filter(([key]) => keys.has(key))),
    optional: example.optional,
  };
};

const withExample = (plan: ImportPlan, example: ImportExample): ImportPlan => ({
  ...plan,
  example,
  pattern: compileExample(example).pattern,
});

/**
 * "Some files have no part": how many more files the pattern would match with
 * the part optional, from a preview of that candidate plan.
 */
const OptionalHint = ({
  detail,
  plan,
  example,
  label,
  matchedNow,
  onChange,
}: TabProps & {
  readonly example: ImportExample;
  readonly label: Label;
  readonly matchedNow: number;
}): ReactElement | null => {
  const { t, number } = useImportText();
  const candidate = withExample(plan, withOptional(example, label, true));
  const state = usePreview(detail.id, {
    include: ['files'],
    files: { filter: 'match', offset: 0, limit: 0 },
    plan: candidate,
  });
  const text = NO_PART[label];
  const more = (state.preview?.files?.matched ?? 0) - matchedNow;
  if (text === undefined || state.preview?.error !== undefined || more <= 0) {
    return null;
  }
  return (
    <p className="import-hint">
      {t(text.missing)}{' '}
      <button type="button" onClick={() => { onChange((current) => withExample(current, withOptional(example, label, true))); }}>
        {t('example.makeOptional')}
      </button>{' '}
      <span className="muted">{t('example.moreMatch', { count: number(more) })}</span>
    </p>
  );
};

const ExampleMode = (
  props: TabProps & { readonly example: ImportExample; readonly matchedNow: number | null; readonly onRegex: () => void },
): ReactElement => {
  const { t } = useImportText();
  const { example, onChange, matchedNow, onRegex } = props;
  const setExample = (next: ImportExample): void => {
    onChange((current) => withExample(current, next));
  };
  return (
    <>
      <p>{t('example.intro')}</p>
      <p>
        {t('example.file')} <span className="import-mono">{example.path}</span>
      </p>
      <ExampleMarker example={example} onChange={setExample} />
      {matchedNow === null
        ? null
        : optionalCandidates(example).map((label) => (
            <OptionalHint key={label} {...props} label={label} matchedNow={matchedNow} />
          ))}
      {optionalInUse(example).map((label) => {
        const text = NO_PART[label];
        return text === undefined ? null : (
          <p key={label} className="import-hint">
            {t(text.optional)}{' '}
            <button type="button" className="link-button" onClick={() => { setExample(withOptional(example, label, false)); }}>
              {t('example.makeRequired')}
            </button>
          </p>
        );
      })}
      <ExampleShape example={example} onEditRegex={onRegex} />
    </>
  );
};

const RegexMode = ({ plan, onChange }: TabProps): ReactElement => {
  const { t } = useImportText();
  const setPattern = (pattern: string): void => {
    onChange((current) => ({ ...current, pattern, example: null }));
  };
  return (
    <>
      <p>{t('regex.intro')}</p>
      <div className="import-chips" role="group" aria-label={t('regex.presets')}>
        <span>{t('regex.presets')}</span>
        {PRESETS.map((preset) => (
          <button
            key={preset.shape}
            type="button"
            className="import-mono"
            aria-pressed={plan.pattern === preset.pattern}
            onClick={() => { setPattern(preset.pattern); }}
          >
            {preset.shape}
          </button>
        ))}
      </div>
      <p>
        <label>
          {t('regex.label')}
          <input
            className="import-mono import-wide"
            spellCheck={false}
            autoComplete="off"
            value={plan.pattern ?? ''}
            onChange={(event) => { setPattern(event.target.value); }}
          />
        </label>
      </p>
    </>
  );
};

/** The first files of the folder, whatever the pattern: the example's candidates. */
const SAMPLE: PreviewRequest = { include: ['files'], files: { filter: 'all', offset: 0, limit: 1 } };

export const FolderStep = (props: TabProps): ReactElement => {
  const { t } = useImportText();
  const { detail, plan, editable, onChange } = props;
  const [mode, setMode] = useState<Mode>(plan.example !== null || plan.pattern === null ? 'example' : 'regex');
  const [view, setView] = useState<FileView>({ filter: 'all', perItem: null });
  const sample = usePreview(detail.id, plan.example === null ? SAMPLE : null);
  const firstPath = sample.preview?.files?.page[0]?.path ?? null;

  // A new draft starts from its first file, marked as one item per file.
  useEffect(() => {
    if (mode === 'example' && plan.example === null && firstPath !== null) {
      onChange((current) => (current.example === null ? withExample(current, defaultExample(firstPath)) : current));
    }
  }, [mode, plan.example, firstPath, onChange]);

  const request: PreviewRequest & { readonly files: FilesQuery } = {
    include: ['files'],
    files: filesQuery(view, 0),
    plan,
  };
  const files = usePreview(detail.id, plan.pattern === null ? null : request);
  const example = plan.example;
  const onUse =
    mode === 'example' && example !== null && editable
      ? (path: string): void => { onChange((current) => withExample(current, moveExample(example, path))); }
      : null;
  return (
    <section className="import-folder">
      <UndoBanner />
      <p className="lede">{t('folder.lede', { folder: detail.folder })}</p>
      <h3>{t('folder.how')}</h3>
      <div className="import-chips" role="group" aria-label={t('folder.modeLabel')}>
        <button type="button" aria-pressed={mode === 'example'} onClick={() => { setMode('example'); }}>
          {t('folder.modeExample')}
        </button>
        <button type="button" aria-pressed={mode === 'regex'} onClick={() => { setMode('regex'); }}>
          {t('folder.modeRegex')}
        </button>
      </div>
      {mode === 'regex' ? (
        <RegexMode {...props} />
      ) : example === null ? (
        <p>{sample.preview === null ? t('archive.loadingPreview') : t('example.noFiles')}</p>
      ) : (
        <ExampleMode
          {...props}
          example={example}
          matchedNow={files.answers === requestKey(request) ? (files.preview?.files?.matched ?? null) : null}
          onRegex={() => { setMode('regex'); }}
        />
      )}
      <PreviewError preview={files.preview} />
      {plan.pattern === null ? null : (
        <FileTable id={detail.id} request={request} first={files} view={view} onView={setView} onUse={onUse} />
      )}
    </section>
  );
};
