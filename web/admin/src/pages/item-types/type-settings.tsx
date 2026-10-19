/**
 * The type-level settings: the standard class line under the name, and the
 * "Used for" hint and the upstream title, description and owner members below
 * the fields.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import type { Property, ResourceClass } from '../../api/types';
import { OptionSelect } from '../../app/fields';
import { userOptions } from '../../app/options';
import { plainName } from '../../app/plain-terms';
import { TermHelp, TermPicker } from '../../app/term-picker';
import { RESOURCE_KINDS, type ResourceKind } from '../../app/type-use';
import { useLocale, useTranslator, type Locale } from '../../i18n';
import { useLocalText } from '../../app/local-messages';
import { TYPES } from './messages';
import type { TypeDraft } from './model';
import type { TypeOptions } from './options';

export const ClassLine = ({
  draft,
  classes,
  onChange,
}: {
  readonly draft: TypeDraft;
  readonly classes: readonly ResourceClass[];
  readonly onChange: (draft: TypeDraft) => void;
}): ReactElement => {
  const { t } = useLocalText(TYPES);
  const locale = useLocale();
  const [changing, setChanging] = useState(false);
  const current = classes.find((entry) => entry.id === draft.resourceClass) ?? null;
  return (
    <div className="type-class">
      <p className="type-class-line">
        {current === null ? (
          t('class.none')
        ) : (
          <>
            {t('class.line', { name: plainName(current, locale) })} <code>{current.term}</code>
            <TermHelp entry={current} />
          </>
        )}
        <button
          type="button"
          className="link-button"
          aria-expanded={changing}
          aria-label={t('class.change')}
          onClick={() => {
            setChanging(!changing);
          }}
        >
          {t('action.change')}
        </button>
      </p>
      {changing ? (
        <div className="type-class-picker">
          <TermPicker
            label={t('class.pick')}
            placeholder={t('class.pickHint')}
            entries={classes}
            autoFocus
            onPick={(picked) => {
              onChange({ ...draft, resourceClass: picked.id });
              setChanging(false);
            }}
          />
          {current === null ? null : (
            <button
              type="button"
              onClick={() => {
                onChange({ ...draft, resourceClass: null });
                setChanging(false);
              }}
            >
              {t('class.clear')}
            </button>
          )}
        </div>
      ) : null}
    </div>
  );
};

const KIND_LABELS = {
  items: 'use.items',
  'item-sets': 'use.itemSets',
  media: 'use.media',
} as const;

const propertyChoices = (properties: readonly Property[], locale: Locale) =>
  properties.map((property) => ({
    id: property.id,
    label: `${plainName(property, locale)} (${property.term})`,
  }));

export interface TypeSettingsProps {
  readonly draft: TypeDraft;
  readonly onChange: (draft: TypeDraft) => void;
  readonly kinds: readonly ResourceKind[];
  readonly onKinds: (kinds: readonly ResourceKind[]) => void;
  readonly options: TypeOptions;
  readonly properties: readonly Property[];
}

export const TypeSettings = ({
  draft,
  onChange,
  kinds,
  onKinds,
  options,
  properties,
}: TypeSettingsProps): ReactElement => {
  const { t } = useLocalText(TYPES);
  const shared = useTranslator();
  const locale = useLocale();
  const choices = propertyChoices(properties, locale);
  return (
    <>
      <fieldset className="type-use">
        <legend>{t('use.legend')}</legend>
        {RESOURCE_KINDS.map((kind) => (
          <label key={kind}>
            <input
              type="checkbox"
              checked={kinds.includes(kind)}
              onChange={(event) => {
                onKinds(
                  event.target.checked ? [...kinds, kind] : kinds.filter((entry) => entry !== kind),
                );
              }}
            />
            {t(KIND_LABELS[kind])}
          </label>
        ))}
        <p className="field-note">{t('use.note')}</p>
      </fieldset>
      <details className="advanced-fold">
        <summary>{t('advanced.summary')}</summary>
        <OptionSelect
          id="type-title-property"
          label={t('advanced.title')}
          options={choices}
          value={draft.titleProperty}
          emptyLabel={t('advanced.titleDefault')}
          onChange={(value) => {
            onChange({ ...draft, titleProperty: value });
          }}
        />
        <OptionSelect
          id="type-description-property"
          label={t('advanced.description')}
          options={choices}
          value={draft.descriptionProperty}
          emptyLabel={t('advanced.descriptionDefault')}
          onChange={(value) => {
            onChange({ ...draft, descriptionProperty: value });
          }}
        />
        <OptionSelect
          id="type-owner"
          label={shared.t('field.owner')}
          options={userOptions(options.users)}
          value={draft.owner}
          emptyLabel={shared.t('common.noOwner')}
          onChange={(value) => {
            onChange({ ...draft, owner: value });
          }}
        />
      </details>
    </>
  );
};
