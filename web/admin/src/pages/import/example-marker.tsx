/**
 * The marked example: one chip per folder and name part, the label choice for
 * the chip in hand, the optional-part hints and the readable shape.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import type { ImportExample, Label } from '../../api/imports';
import { compileExample, folderKey, labelsFor, splitExample, withLabel } from './example';
import { LABEL_TEXT, tagClass } from './labels';
import { useImportText } from './messages';

const Chip = ({
  text,
  label,
  selected,
  onSelect,
}: {
  readonly text: string;
  readonly label: Label | undefined;
  readonly selected: boolean;
  readonly onSelect: () => void;
}): ReactElement => {
  const { t } = useImportText();
  return (
    <button
      type="button"
      className={label === undefined ? 'import-chip' : `import-chip ${tagClass(label)}`}
      aria-pressed={selected}
      onClick={onSelect}
    >
      <span className="import-mono">{text}</span>
      <small>{label === undefined ? ' ' : t(LABEL_TEXT[label].name)}</small>
    </button>
  );
};

const LabelChoice = ({
  text,
  choices,
  current,
  onChoose,
}: {
  readonly text: string;
  readonly choices: readonly Label[];
  readonly current: Label | undefined;
  readonly onChoose: (label: Label | null) => void;
}): ReactElement => {
  const { t } = useImportText();
  return (
    <div className="import-chips" role="group" aria-label={t('example.is', { text })}>
      <span>{t('example.is', { text })}</span>
      {choices.map((choice) => (
        <button
          key={choice}
          type="button"
          title={t(LABEL_TEXT[choice].help)}
          aria-pressed={current === choice}
          onClick={() => { onChoose(choice); }}
        >
          {t(LABEL_TEXT[choice].name)}
        </button>
      ))}
      <button
        type="button"
        title={t('help.nothing')}
        aria-pressed={current === undefined}
        onClick={() => { onChoose(null); }}
      >
        {t('label.none')}
      </button>
    </div>
  );
};

export const ExampleMarker = ({
  example,
  onChange,
}: {
  readonly example: ImportExample;
  readonly onChange: (example: ImportExample) => void;
}): ReactElement => {
  const { t } = useImportText();
  const [selected, setSelected] = useState<string | null>(null);
  const parts = splitExample(example.path);
  const textOf = (key: string): string =>
    key.startsWith('d')
      ? (parts.folders[Number(key.slice(1))] ?? '')
      : (parts.name.find((part) => part.key === key)?.text ?? '');
  const chip = (key: string, text: string): ReactElement => (
    <Chip
      key={key}
      text={text}
      label={example.labels[key]}
      selected={selected === key}
      onSelect={() => { setSelected(selected === key ? null : key); }}
    />
  );
  return (
    <>
      <div className="import-example" role="group" aria-label={t('example.partsLabel')}>
        {parts.folders.map((folder, index) => (
          <span key={folderKey(index)} className="import-example-folder">
            {chip(folderKey(index), folder)}
            <span className="import-sep">/</span>
          </span>
        ))}
        {parts.name.map((part) =>
          part.delimiter ? (
            <span key={part.key} className="import-sep">
              {part.text}
            </span>
          ) : (
            chip(part.key, part.text)
          ),
        )}
        {parts.ext === '' ? null : (
          <>
            <span className="import-sep">.</span>
            <span className="import-mono import-tag import-tag-ext">{parts.ext}</span>
          </>
        )}
      </div>
      {selected === null ? null : (
        <LabelChoice
          text={textOf(selected)}
          choices={labelsFor(selected)}
          current={example.labels[selected]}
          onChoose={(label) => { onChange(withLabel(example, selected, label)); }}
        />
      )}
    </>
  );
};

export const ExampleShape = ({
  example,
  onEditRegex,
}: {
  readonly example: ImportExample;
  readonly onEditRegex: () => void;
}): ReactElement => {
  const { t } = useImportText();
  return (
    <p className="query-count">
      {t('example.shape')} <span className="import-mono">{compileExample(example).shape}</span>.{' '}
      {t('example.shapeNote')}{' '}
      <button type="button" className="link-button" onClick={onEditRegex}>
        {t('example.editRegex')}
      </button>
    </p>
  );
};
