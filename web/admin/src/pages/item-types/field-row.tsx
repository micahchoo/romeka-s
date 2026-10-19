/**
 * One field of an item type: its name on the form, the standard term behind
 * it, what it accepts, whether it is required or private, and its help text
 * and language. Each smart default is shown and has its override in place.
 */
import { Fragment, useEffect, useId, useRef, useState } from 'react';
import type { DragEvent, KeyboardEvent, ReactElement } from 'react';
import { useTermText } from '../../i18n/termComments';
import type { DataType, Property } from '../../api/types';
import { useLesson } from '../../app/lessons/lesson';
import { TermHelp, StandardTermPicker } from '../../app/standard-term-picker';
import { isTermListName } from '../../api/term-lists';
import { acceptChoices, acceptGroups, CORE_ACCEPTS, isCoreAccepts, isInstalled } from './accepts';
import { useItemTypesText, type ItemTypesText } from './messages';
import { patchField, textOrNull, toggleDataType, type FieldDraft, type TypeDraft } from './model';

export const acceptLabel = (
  type: string,
  dataTypes: readonly DataType[],
  t: ItemTypesText,
): string => {
  if (isCoreAccepts(type)) {
    return t(CORE_ACCEPTS[type]);
  }
  const registered = dataTypes.find((dataType) => dataType.id === type);
  if (registered === undefined) {
    return t('accepts.missing', { id: type });
  }
  return isTermListName(type) ? t('accepts.termList', { list: registered.label }) : registered.label;
};

/** A choice's label inside its group, where the group's legend already says "Term lists". */
const choiceLabel = (type: string, dataTypes: readonly DataType[], t: ItemTypesText): string =>
  isTermListName(type)
    ? (dataTypes.find((dataType) => dataType.id === type)?.label ?? t('accepts.missing', { id: type }))
    : acceptLabel(type, dataTypes, t);

/** The drag payload type, so a drop only accepts a field of this editor. */
export const FIELD_DRAG = 'application/x-romeka-type-field';

/** The lesson a row's standard term anchors, if any (`FieldList` picks the rows). */
export type TermLesson = 'term' | 'local-field' | null;

export interface FieldRowProps {
  readonly field: FieldDraft;
  readonly index: number;
  readonly count: number;
  readonly draft: TypeDraft;
  readonly onChange: (draft: TypeDraft) => void;
  readonly onMove: (key: string, index: number) => void;
  readonly onRemove: (key: string) => void;
  /** Set while this row's handle should take focus after a move. */
  readonly focusHandle: boolean;
  readonly properties: readonly Property[];
  readonly freeProperties: readonly Property[];
  readonly dataTypes: readonly DataType[];
  readonly createField: (name: string) => Promise<Property>;
  readonly termLesson: TermLesson;
}

const AcceptsEditor = ({
  field,
  dataTypes,
  name,
  onToggle,
}: {
  readonly field: FieldDraft;
  readonly dataTypes: readonly DataType[];
  readonly name: string;
  readonly onToggle: (type: string, on: boolean) => void;
}): ReactElement => {
  const t = useItemTypesText();
  const choice = (type: string): ReactElement => (
    <label key={type}>
      <input
        type="checkbox"
        checked={field.dataTypes.includes(type)}
        onChange={(event) => {
          onToggle(type, event.target.checked);
        }}
      />
      {choiceLabel(type, dataTypes, t)}
      {isInstalled(dataTypes, type) || isCoreAccepts(type) ? null : (
        <code className="type-field-type-id">{type}</code>
      )}
    </label>
  );
  return (
    <fieldset className="type-field-accepts-list">
      <legend>{t('accepts.legend', { name })}</legend>
      {acceptGroups(acceptChoices(dataTypes, field.dataTypes)).map((entry) =>
        entry.group === 'kinds' ? (
          <Fragment key={entry.group}>{entry.types.map(choice)}</Fragment>
        ) : (
          <fieldset key={entry.group} className="type-field-accepts-group">
            <legend>{t('accepts.termLists')}</legend>
            {entry.types.map(choice)}
          </fieldset>
        ),
      )}
      <p className="field-note">{t('accepts.note')}</p>
    </fieldset>
  );
};

interface SettingsProps {
  readonly field: FieldDraft;
  readonly property: Property | undefined;
  readonly term: string;
  /** The name the form shows, for the controls' accessible names. */
  readonly name: string;
  readonly patch: (next: Partial<Omit<FieldDraft, 'key'>>) => void;
  readonly freeProperties: readonly Property[];
  readonly dataTypes: readonly DataType[];
  readonly createField: (name: string) => Promise<Property>;
  readonly termLesson: TermLesson;
}

/** The standard term behind the field, and the picker that changes it. */
const TermLine = ({
  property,
  term,
  name: shownName,
  patch,
  freeProperties,
  createField,
  termLesson,
}: SettingsProps): ReactElement => {
  const t = useItemTypesText();
  const [changing, setChanging] = useState(false);
  const lesson = useLesson(termLesson ?? 'term', termLesson !== null);
  return (
    <>
      <p className="type-field-term">
        <code>{term}</code>
        {property === undefined ? null : <TermHelp entry={property} />}
        {lesson.toggle}
        <button
          type="button"
          className="link-button"
          aria-expanded={changing}
          aria-label={t('field.changeTerm', { name: shownName })}
          onClick={() => {
            setChanging(!changing);
          }}
        >
          {t('action.change')}
        </button>
      </p>
      {lesson.panel}
      {changing ? (
        <StandardTermPicker
          label={t('field.pickTerm')}
          placeholder={t('field.pickHint')}
          entries={freeProperties}
          autoFocus
          onPick={(picked) => {
            patch({ propertyId: picked.id });
            setChanging(false);
          }}
          onCreate={async (name) => {
            patch({ propertyId: (await createField(name)).id });
            setChanging(false);
          }}
        />
      ) : null}
    </>
  );
};

/** What the field accepts, summed up, and the list that changes it. */
const AcceptsLine = ({ field, name: shownName, patch, dataTypes }: SettingsProps): ReactElement => {
  const t = useItemTypesText();
  const id = useId();
  const [accepting, setAccepting] = useState(false);
  return (
    <>
      <p className="type-field-accepts">
        <span>
          {t('accepts.label')}{' '}
          <strong>
            {field.dataTypes.length === 0
              ? t('accepts.any')
              : field.dataTypes.map((type) => acceptLabel(type, dataTypes, t)).join(', ')}
          </strong>
        </span>
        <button
          type="button"
          className="link-button"
          aria-expanded={accepting}
          aria-controls={`${id}-accepts`}
          aria-label={t('accepts.change', { name: shownName })}
          onClick={() => {
            setAccepting(!accepting);
          }}
        >
          {t('action.change')}
        </button>
      </p>
      <div id={`${id}-accepts`} hidden={!accepting}>
        <AcceptsEditor
          field={field}
          dataTypes={dataTypes}
          name={shownName}
          onToggle={(type, on) => {
            patch({ dataTypes: toggleDataType(field, type, on) });
          }}
        />
      </div>
    </>
  );
};

/** Required, private, and the help text and language under a fold. */
const FlagsLine = ({ field, property, name: shownName, patch }: SettingsProps): ReactElement => {
  const t = useItemTypesText();
  const text = useTermText();
  const id = useId();
  return (
    <>
      <p className="type-field-flags">
        <label>
          <input
            type="checkbox"
            aria-label={t('field.required', { name: shownName })}
            checked={field.isRequired}
            onChange={(event) => {
              patch({ isRequired: event.target.checked });
            }}
          />
          {t('field.requiredShort')}
        </label>
        <label>
          <input
            type="checkbox"
            aria-label={t('field.private', { name: shownName })}
            checked={field.isPrivate}
            onChange={(event) => {
              patch({ isPrivate: event.target.checked });
            }}
          />
          {t('field.privateShort')}
        </label>
      </p>
      <details className="type-field-more">
        <summary>{t('field.more')}</summary>
        <p>
          <label htmlFor={`${id}-help`}>{t('field.help')}</label>
          <input
            id={`${id}-help`}
            type="text"
            placeholder={(property === undefined ? null : text.comment(property.term, property.comment)) ?? ''}
            value={field.alternateComment ?? ''}
            onChange={(event) => {
              patch({ alternateComment: textOrNull(event.target.value) });
            }}
          />
        </p>
        <p>
          <label htmlFor={`${id}-lang`}>{t('itemTypes.fieldLanguage')}</label>
          <input
            id={`${id}-lang`}
            type="text"
            className="type-field-lang"
            value={field.defaultLang ?? ''}
            onChange={(event) => {
              patch({ defaultLang: textOrNull(event.target.value) });
            }}
          />
        </p>
      </details>
    </>
  );
};

const FieldSettings = (props: SettingsProps): ReactElement => {
  const t = useItemTypesText();
  const text = useTermText();
  const { field, property, term, patch } = props;
  const [renamed, setRenamed] = useState(false);
  const lesson = useLesson('rename', renamed);
  return (
    <div className="type-field-body">
      <div className="lesson-line">
        <input
          className="type-field-name"
          type="text"
          aria-label={t('field.name', { term })}
          placeholder={(property === undefined ? null : text.label(property.term, property.label)) ?? term}
          value={field.alternateLabel ?? ''}
          onChange={(event) => {
            setRenamed(true);
            patch({ alternateLabel: textOrNull(event.target.value) });
          }}
        />
        {lesson.toggle}
      </div>
      {lesson.panel}
      <TermLine {...props} />
      <AcceptsLine {...props} />
      <FlagsLine {...props} />
    </div>
  );
};

interface RowActionsProps {
  readonly field: FieldDraft;
  readonly index: number;
  readonly count: number;
  readonly name: string;
  readonly onMove: (key: string, index: number) => void;
  readonly onRemove: (key: string) => void;
}

/** Up, down and remove: the buttons beside the card. */
const RowActions = ({
  field,
  index,
  count,
  name: shownName,
  onMove,
  onRemove,
}: RowActionsProps): ReactElement => {
  const t = useItemTypesText();
  return (
    <div className="type-field-actions">
      <button
        type="button"
        aria-label={t('field.moveUp', { name: shownName })}
        disabled={index === 0}
        onClick={() => {
          onMove(field.key, index - 1);
        }}
      >
        <span aria-hidden="true">↑</span>
      </button>
      <button
        type="button"
        aria-label={t('field.moveDown', { name: shownName })}
        disabled={index === count - 1}
        onClick={() => {
          onMove(field.key, index + 1);
        }}
      >
        <span aria-hidden="true">↓</span>
      </button>
      <button
        type="button"
        aria-label={t('field.remove', { name: shownName })}
        onClick={() => {
          onRemove(field.key);
        }}
      >
        <span aria-hidden="true">×</span>
      </button>
    </div>
  );
};

export const FieldRow = ({
  field,
  index,
  count,
  draft,
  onChange,
  onMove,
  onRemove,
  focusHandle,
  properties,
  freeProperties,
  dataTypes,
  createField,
  termLesson,
}: FieldRowProps): ReactElement => {
  const t = useItemTypesText();
  const text = useTermText();
  const handle = useRef<HTMLButtonElement>(null);
  const property = properties.find((entry) => entry.id === field.propertyId);
  const term = property?.term ?? `#${String(field.propertyId)}`;
  const shownName =
    field.alternateLabel ?? (property === undefined ? null : text.label(property.term, property.label)) ?? term;
  const patch = (next: Partial<Omit<FieldDraft, 'key'>>): void => {
    onChange(patchField(draft, field.key, next));
  };

  useEffect(() => {
    if (focusHandle) {
      handle.current?.focus();
    }
  }, [focusHandle, index]);

  const onHandleKey = (event: KeyboardEvent<HTMLButtonElement>): void => {
    const target =
      event.key === 'ArrowUp' ? index - 1 : event.key === 'ArrowDown' ? index + 1 : null;
    if (target === null || target < 0 || target >= count) {
      return;
    }
    event.preventDefault();
    onMove(field.key, target);
  };
  const onDrop = (event: DragEvent<HTMLLIElement>): void => {
    const key = event.dataTransfer.getData(FIELD_DRAG);
    if (key !== '') {
      event.preventDefault();
      onMove(key, index);
    }
  };

  return (
    <li
      className="type-field"
      onDragOver={(event) => {
        if (event.dataTransfer.types.includes(FIELD_DRAG)) {
          event.preventDefault();
        }
      }}
      onDrop={onDrop}
    >
      <button
        ref={handle}
        type="button"
        className="type-field-handle"
        draggable
        aria-label={t('field.moveHandle', { name: shownName, position: index + 1, count })}
        onDragStart={(event) => {
          event.dataTransfer.setData(FIELD_DRAG, field.key);
          event.dataTransfer.effectAllowed = 'move';
        }}
        onKeyDown={onHandleKey}
      >
        <span aria-hidden="true">⠿</span>
      </button>
      <FieldSettings
        field={field}
        property={property}
        term={term}
        name={shownName}
        patch={patch}
        freeProperties={freeProperties}
        dataTypes={dataTypes}
        createField={createField}
        termLesson={termLesson}
      />
      <RowActions
        field={field}
        index={index}
        count={count}
        name={shownName}
        onMove={onMove}
        onRemove={onRemove}
      />
    </li>
  );
};
