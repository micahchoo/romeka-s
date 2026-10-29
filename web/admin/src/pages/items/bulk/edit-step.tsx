/**
 * The panel's first step: which items, one row per field the change can set, and the value
 * changes (`values-section.tsx`). Every row starts at "No change", so the change names only what
 * the person chose.
 */
import type { ReactElement } from 'react';
import { Suspense, lazy, useId, useState } from 'react';
import type { Assign, BulkEdit, Membership } from '../../../api/bulk';
import { namesSomething } from '../../../api/bulk';
import { useTranslator } from '../../../i18n';
import { bulk } from '../../../i18n/namespaces/bulk';
import { useClassNamesOf } from '../browse-options';
import type { BrowseOptions } from '../browse-options';
import type { Draft } from './model';
import { ScopeChoice } from './scope-choice';
import type { Reach } from './scope-choice';
import { NEW_COMPOSER, isWriting } from './values-form';
import type { Composer } from './values-model';
import { classNameOf } from './words';

/** The Values section is its own chunk, so the panel stays inside its budget (`perf/budget.json`). */
const ValuesSection = lazy(() => import('./values-section').then((module) => ({ default: module.ValuesSection })));

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
  const classNames = useClassNamesOf(options.classes);
  const { edit } = draft;
  const [composer, setComposer] = useState<Composer>(NEW_COMPOSER);
  const set = (change: Partial<BulkEdit>): void => {
    onDraft({ ...draft, edit: { ...edit, ...change } });
  };
  const keep: Choice = { value: KEEP, label: t('bulk.choice.keep') };
  const classChoices = [...classNames.keys()]
    .map((id) => ({ value: String(id), label: classNameOf({ t, options, classNames }, id) }))
    .sort(byLabel);
  const writing = isWriting(composer);
  const ready = !writing && namesSomething(edit) && (draft.scope === 'ticked' ? reach.ticked > 0 : reach.canMatchAll);
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
        refused={t('bulk.scope.matchingRefused')}
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
            set({ class: assignOf(classNames, value) });
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
      <Suspense fallback={<p className="bulk-note">{t('bulk.values.loading')}</p>}>
        <ValuesSection
          ops={edit.values}
          onOps={(values) => {
            set({ values });
          }}
          composer={composer}
          onComposer={setComposer}
        />
      </Suspense>
      {ready ? null : <p className="bulk-note">{t(writing ? 'bulk.edit.unadded' : 'bulk.edit.nothing')}</p>}
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
