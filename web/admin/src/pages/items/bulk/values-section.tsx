/**
 * The edit panel's Values section: the value changes chosen so far, each in words with a Remove
 * button, and a small form that writes one more. The form asks for a field with the plain-words
 * search the item form uses, then what to do with it, then the inputs that action needs. The edit
 * step holds the form, because Preview waits while the form holds a change not yet added.
 */
import type { ReactElement, ReactNode } from 'react';
import { useId } from 'react';
import { api } from '../../../api/client';
import type { ValueOp } from '../../../api/bulk-values';
import { withOp } from '../../../api/bulk-values';
import type { Property, PropertyId } from '../../../api/types';
import { ALL_PAGES_PER_PAGE, allPages } from '../../../app/all-pages';
import { ErrorNotice } from '../../../app/errors';
import { plainName } from '../../../app/plain-terms';
import { StandardTermPicker } from '../../../app/standard-term-picker';
import { useAsync } from '../../../app/useAsync';
import { useTranslator } from '../../../i18n';
import { bulk } from '../../../i18n/namespaces/bulk';
import { useTermLabels } from '../../../i18n/termText';
import { NEW_COMPOSER } from './values-form';
import { ACTIONS, KINDS, opOf } from './values-model';
import type { Composer, ValueAction, ValueKind } from './values-model';
import type { BulkTranslate } from './words';
import { opWords } from './words';

const loadProperties = (): Promise<readonly Property[]> =>
  allPages((page) => api.listProperties({ page, perPage: ALL_PAGES_PER_PAGE, sortBy: 'label' }));

const ACTION_WORDS = {
  add: 'bulk.values.add',
  replace: 'bulk.values.replace',
  clear: 'bulk.values.clear',
  visibility: 'bulk.values.visibility',
} as const satisfies Record<ValueAction, string>;

const KIND_WORDS = {
  literal: 'bulk.values.kindText',
  uri: 'bulk.values.kindUri',
  item: 'bulk.values.kindItem',
} as const satisfies Record<ValueKind, string>;

/** A labelled control; `render` gets the id the label points at. */
const Labelled = ({
  label,
  render,
}: {
  readonly label: string;
  readonly render: (id: string) => ReactNode;
}): ReactElement => {
  const id = useId();
  return (
    <p className="bulk-field">
      <label htmlFor={id}>{label}</label>
      {render(id)}
    </p>
  );
};

const Text = ({
  label,
  value,
  onText,
  inputMode,
}: {
  readonly label: string;
  readonly value: string;
  readonly onText: (text: string) => void;
  readonly inputMode?: 'numeric' | 'url';
}): ReactElement => (
  <Labelled
    label={label}
    render={(id) => (
      <input
        id={id}
        value={value}
        inputMode={inputMode}
        onChange={(event) => {
          onText(event.target.value);
        }}
      />
    )}
  />
);

const Choose = <T extends string>({
  label,
  value,
  choices,
  onChoose,
}: {
  readonly label: string;
  readonly value: T;
  readonly choices: readonly (readonly [T, string])[];
  readonly onChoose: (value: T) => void;
}): ReactElement => (
  <Labelled
    label={label}
    render={(id) => (
      <select
        id={id}
        value={value}
        onChange={(event) => {
          const chosen = choices.find(([key]) => key === event.target.value);
          if (chosen !== undefined) {
            onChoose(chosen[0]);
          }
        }}
      >
        {choices.map(([key, words]) => (
          <option key={key} value={key}>
            {words}
          </option>
        ))}
      </select>
    )}
  />
);

/** The inputs one action needs. */
const ActionInputs = ({
  t,
  composer,
  set,
}: {
  readonly t: BulkTranslate;
  readonly composer: Composer;
  readonly set: (change: Partial<Composer>) => void;
}): ReactElement => {
  const shown = (label: string): ReactElement => (
    <Choose
      label={label}
      value={composer.isPublic ? 'public' : 'private'}
      choices={[
        ['public', t('bulk.choice.public')],
        ['private', t('bulk.choice.private')],
      ]}
      onChoose={(value) => {
        set({ isPublic: value === 'public' });
      }}
    />
  );
  switch (composer.action) {
    case 'clear':
      return <p className="bulk-note">{t('bulk.values.clearNote')}</p>;
    case 'visibility':
      return shown(t('bulk.values.makeShown'));
    case 'replace':
      return (
        <>
          <Text label={t('bulk.values.find')} value={composer.from} onText={(from) => { set({ from }); }} />
          <Text label={t('bulk.values.put')} value={composer.to} onText={(to) => { set({ to }); }} />
        </>
      );
    case 'add':
      return (
        <>
          <Choose
            label={t('bulk.values.kind')}
            value={composer.kind}
            choices={KINDS.map((kind) => [kind, t(KIND_WORDS[kind])] as const)}
            onChoose={(kind) => {
              set({ kind });
            }}
          />
          {composer.kind === 'literal' ? (
            <>
              <Text label={t('bulk.values.text')} value={composer.text} onText={(text) => { set({ text }); }} />
              <Text label={t('bulk.values.lang')} value={composer.lang} onText={(lang) => { set({ lang }); }} />
            </>
          ) : null}
          {composer.kind === 'uri' ? (
            <>
              <Text label={t('bulk.values.uri')} value={composer.uri} inputMode="url" onText={(uri) => { set({ uri }); }} />
              <Text label={t('bulk.values.label')} value={composer.label} onText={(label) => { set({ label }); }} />
            </>
          ) : null}
          {composer.kind === 'item' ? (
            <Text label={t('bulk.values.item')} value={composer.item} inputMode="numeric" onText={(item) => { set({ item }); }} />
          ) : null}
          {shown(t('bulk.values.shown'))}
        </>
      );
  }
};

export const ValuesSection = ({
  ops,
  onOps,
  composer,
  onComposer,
}: {
  readonly ops: readonly ValueOp[];
  readonly onOps: (ops: readonly ValueOp[]) => void;
  readonly composer: Composer;
  readonly onComposer: (composer: Composer) => void;
}): ReactElement => {
  const { t } = useTranslator(bulk);
  const text = useTermLabels();
  const properties = useAsync(loadProperties, []);
  const set = (change: Partial<Composer>): void => {
    onComposer({ ...composer, ...change });
  };
  const list = properties.status === 'ready' ? properties.data : [];
  const nameOf = (id: PropertyId): string => {
    const property = list.find((entry) => entry.id === id);
    return property === undefined ? `#${String(id)}` : plainName(property, text);
  };
  const op = opOf(composer);
  return (
    <fieldset className="bulk-values">
      <legend>{t('bulk.values.legend')}</legend>
      {ops.length === 0 ? null : (
        <ul className="bulk-value-ops">
          {ops.map((entry, at) => {
            const words = opWords(t, nameOf(entry.property), entry);
            return (
              <li key={words}>
                {words}{' '}
                <button
                  type="button"
                  className="link-button"
                  aria-label={t('bulk.values.removeOne', { change: words })}
                  onClick={() => {
                    onOps(ops.filter((_, index) => index !== at));
                  }}
                >
                  {t('bulk.values.remove')}
                </button>
              </li>
            );
          })}
        </ul>
      )}
      {properties.status === 'error' ? <ErrorNotice error={properties.error} /> : null}
      {properties.status === 'loading' ? <p className="bulk-note">{t('bulk.values.loading')}</p> : null}
      {properties.status === 'ready' && composer.property === null ? (
        <StandardTermPicker
          label={t('bulk.values.field')}
          placeholder={t('bulk.values.fieldHint')}
          entries={list}
          onPick={(property) => {
            set({ property: property.id });
          }}
        />
      ) : null}
      {composer.property === null ? null : (
        <div className="bulk-value-form">
          <p>
            {t('bulk.values.fieldChosen', { field: nameOf(composer.property) })}{' '}
            <button
              type="button"
              className="link-button"
              onClick={() => {
                set({ property: null });
              }}
            >
              {t('bulk.values.otherField')}
            </button>
          </p>
          <Choose
            label={t('bulk.values.action')}
            value={composer.action}
            choices={ACTIONS.map((action) => [action, t(ACTION_WORDS[action])] as const)}
            onChoose={(action) => {
              set({ action });
            }}
          />
          <ActionInputs t={t} composer={composer} set={set} />
          <p>
            <button
              type="button"
              disabled={op === null}
              onClick={() => {
                if (op !== null) {
                  onOps(withOp(ops, op));
                  onComposer(NEW_COMPOSER);
                }
              }}
            >
              {t('bulk.values.addChange')}
            </button>{' '}
            <button
              type="button"
              className="link-button"
              onClick={() => {
                onComposer(NEW_COMPOSER);
              }}
            >
              {t('bulk.values.discard')}
            </button>
          </p>
        </div>
      )}
    </fieldset>
  );
};
