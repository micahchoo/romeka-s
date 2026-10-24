/**
 * The panel's first step: which items, and one row per field the change can set. Every row
 * starts at "No change", so the change names only what the person chose.
 */
import type { ReactElement } from 'react';
import { useId } from 'react';
import type { Assign, BulkEdit, Membership } from '../../../api/bulk';
import { namesSomething } from '../../../api/bulk';
import { useTranslator } from '../../../i18n';
import { bulk } from '../../../i18n/namespaces/bulk';
import { useTermLabels } from '../../../i18n/termText';
import type { BrowseOptions } from '../browse-options';
import type { Draft, Scope } from './model';
import { classNameOf } from './words';

/** The two selections the panel can name, with their sizes. */
export interface Reach {
  readonly ticked: number;
  readonly matching: number;
  /** The list has a filter, so "all that match" is not all items. */
  readonly filtered: boolean;
  /** The person's role may change every matching item (`batch_update_all`). */
  readonly canMatchAll: boolean;
}

interface Choice {
  readonly value: string;
  readonly label: string;
}

const KEEP = '';
const CLEAR = 'clear';

const byLabel = (a: Choice, b: Choice): number => a.label.localeCompare(b.label);

const choicesOf = <K,>(map: ReadonlyMap<K, string>): readonly Choice[] =>
  [...map].map(([key, label]) => ({ value: String(key), label })).sort(byLabel);

const keyOf = <K,>(map: ReadonlyMap<K, unknown>, value: string): K | null =>
  [...map.keys()].find((key) => String(key) === value) ?? null;

const Row = ({
  label,
  value,
  choices,
  onChoose,
}: {
  readonly label: string;
  readonly value: string;
  readonly choices: readonly Choice[];
  readonly onChoose: (value: string) => void;
}): ReactElement => {
  const id = useId();
  return (
    <p className="bulk-field">
      <label htmlFor={id}>{label}</label>
      <select
        id={id}
        value={value}
        onChange={(event) => {
          onChoose(event.target.value);
        }}
      >
        {choices.map((choice) => (
          <option key={choice.value} value={choice.value}>
            {choice.label}
          </option>
        ))}
      </select>
    </p>
  );
};

const assignValue = <T extends number>(assign: Assign<T> | null): string =>
  assign === null ? KEEP : assign.kind === 'clear' ? CLEAR : String(assign.id);

const assignOf = <T,>(map: ReadonlyMap<T, unknown>, value: string): Assign<T> | null => {
  if (value === CLEAR) {
    return { kind: 'clear' };
  }
  const id = keyOf(map, value);
  return id === null ? null : { kind: 'set', id };
};

/** One membership row sets one id to add or remove, and takes it out of the other side. */
const withMember = <T,>(members: Membership<T>, side: 'add' | 'remove', id: T | null): Membership<T> => {
  const chosen = id === null ? [] : [id];
  return side === 'add'
    ? { add: chosen, remove: members.remove.filter((entry) => entry !== id) }
    : { add: members.add.filter((entry) => entry !== id), remove: chosen };
};

const MembershipRows = <T extends number>({
  labels,
  members,
  map,
  keep,
  onMembers,
}: {
  readonly labels: readonly [string, string];
  readonly members: Membership<T>;
  readonly map: ReadonlyMap<T, string>;
  readonly keep: Choice;
  readonly onMembers: (members: Membership<T>) => void;
}): ReactElement => (
  <>
    {(['add', 'remove'] as const).map((side) => (
      <Row
        key={side}
        label={side === 'add' ? labels[0] : labels[1]}
        value={String(members[side][0] ?? KEEP)}
        choices={[keep, ...choicesOf(map)]}
        onChoose={(value) => {
          onMembers(withMember(members, side, keyOf(map, value)));
        }}
      />
    ))}
  </>
);

const ScopeChoice = ({
  scope,
  reach,
  onScope,
}: {
  readonly scope: Scope;
  readonly reach: Reach;
  readonly onScope: (scope: Scope) => void;
}): ReactElement => {
  const { t, number } = useTranslator(bulk);
  const tickedText = reach.ticked === 1 ? t('bulk.scope.tickedOne') : t('bulk.scope.tickedMany', { count: number(reach.ticked) });
  const many = reach.filtered ? 'bulk.scope.matchingMany' : 'bulk.scope.allMany';
  const one = reach.filtered ? 'bulk.scope.matchingOne' : 'bulk.scope.allOne';
  const matchingText = reach.matching === 1 ? t(one) : t(many, { count: number(reach.matching) });
  const radio = (value: Scope, text: string, disabled: boolean): ReactElement => (
    <label>
      <input
        type="radio"
        name="bulk-scope"
        value={value}
        checked={scope === value}
        disabled={disabled}
        onChange={() => {
          onScope(value);
        }}
      />{' '}
      {text}
    </label>
  );
  return (
    <fieldset className="bulk-scope">
      <legend>{t('bulk.scope.legend')}</legend>
      {reach.ticked > 0 ? radio('ticked', tickedText, false) : null}
      {radio('matching', matchingText, !reach.canMatchAll)}
      {reach.canMatchAll ? null : <p className="bulk-note">{t('bulk.scope.matchingRefused')}</p>}
    </fieldset>
  );
};

export const EditStep = ({
  draft,
  reach,
  options,
  busy,
  onDraft,
  onPreview,
  onCancel,
}: {
  readonly draft: Draft;
  readonly reach: Reach;
  readonly options: BrowseOptions;
  readonly busy: boolean;
  readonly onDraft: (draft: Draft) => void;
  readonly onPreview: () => void;
  readonly onCancel: () => void;
}): ReactElement => {
  const { t } = useTranslator(bulk);
  const text = useTermLabels();
  const { edit } = draft;
  const set = (change: Partial<BulkEdit>): void => {
    onDraft({ ...draft, edit: { ...edit, ...change } });
  };
  const keep: Choice = { value: KEEP, label: t('bulk.choice.keep') };
  const classChoices = [...options.classNames.keys()]
    .map((id) => ({ value: String(id), label: classNameOf({ t, options, text }, id) }))
    .sort(byLabel);
  const ready = namesSomething(edit) && (draft.scope === 'ticked' ? reach.ticked > 0 : reach.canMatchAll);
  return (
    <form
      className="bulk-edit"
      onSubmit={(event) => {
        event.preventDefault();
        if (ready) {
          onPreview();
        }
      }}
    >
      <ScopeChoice
        scope={draft.scope}
        reach={reach}
        onScope={(scope) => {
          onDraft({ ...draft, scope });
        }}
      />
      <div className="bulk-fields">
        <Row
          label={t('bulk.field.visibility')}
          value={edit.isPublic === null ? KEEP : edit.isPublic ? 'public' : 'private'}
          choices={[keep, { value: 'public', label: t('bulk.choice.public') }, { value: 'private', label: t('bulk.choice.private') }]}
          onChoose={(value) => {
            set({ isPublic: value === KEEP ? null : value === 'public' });
          }}
        />
        <Row
          label={t('bulk.field.owner')}
          value={edit.owner === null ? KEEP : String(edit.owner)}
          choices={[keep, ...choicesOf(options.users)]}
          onChoose={(value) => {
            set({ owner: keyOf(options.users, value) });
          }}
        />
        <Row
          label={t('bulk.field.class')}
          value={assignValue(edit.class)}
          choices={[keep, { value: CLEAR, label: t('bulk.choice.noClass') }, ...classChoices]}
          onChoose={(value) => {
            set({ class: assignOf(options.classNames, value) });
          }}
        />
        <Row
          label={t('bulk.field.template')}
          value={assignValue(edit.template)}
          choices={[keep, { value: CLEAR, label: t('bulk.choice.noTemplate') }, ...choicesOf(options.templates)]}
          onChoose={(value) => {
            set({ template: assignOf(options.templates, value) });
          }}
        />
        <MembershipRows
          labels={[t('bulk.field.addSet'), t('bulk.field.removeSet')]}
          members={edit.itemSets}
          map={options.itemSets}
          keep={keep}
          onMembers={(itemSets) => {
            set({ itemSets });
          }}
        />
        <MembershipRows
          labels={[t('bulk.field.addSite'), t('bulk.field.removeSite')]}
          members={edit.sites}
          map={options.sites}
          keep={keep}
          onMembers={(sites) => {
            set({ sites });
          }}
        />
      </div>
      {ready ? null : <p className="bulk-note">{t('bulk.edit.nothing')}</p>}
      <p className="bulk-actions">
        <button type="submit" disabled={!ready || busy} aria-busy={busy}>
          {t('bulk.edit.preview')}
        </button>
        <button type="button" className="link-button" onClick={onCancel}>
          {t('bulk.edit.cancel')}
        </button>
      </p>
    </form>
  );
};
