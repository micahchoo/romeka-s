/**
 * The `repeat` field control: a list of records, each holding the sub-fields the
 * schema names (`blockField.asset.attachments` is the one in the registry). The
 * whole list is the field's value, so one row edit writes the list through
 * `onChange`; the sub-controls are rendered by the caller, which owns the issue
 * and site context.
 */
import type { ReactElement } from 'react';
import type { JsonValue } from '../../../../api/types';
import { useTranslator } from '../../../../i18n';
import type { BlockField } from '../../blockSchema';
import { isJsonObject } from '../values';
import { common } from '../../../../i18n/namespaces/common';
import { editor } from '../../../../i18n/namespaces/editor';

/** The props one sub-control of a repeat entry receives. */
export interface NestedControlProps {
  readonly field: BlockField;
  readonly value: JsonValue | undefined;
  readonly onChange: (value: JsonValue) => void;
  readonly path: string;
  /** The entry the field belongs to, and the entry's fields. */
  readonly record: Readonly<Record<string, JsonValue>>;
  readonly siblings: readonly BlockField[];
  readonly onPatch: (values: Readonly<Record<string, JsonValue>>) => void;
}

export type RenderControl = (props: NestedControlProps) => ReactElement;

export interface RepeatFieldProps {
  readonly label: string;
  readonly fields: readonly BlockField[];
  readonly value: readonly JsonValue[];
  readonly onChange: (value: readonly JsonValue[]) => void;
  readonly renderControl: RenderControl;
  readonly path: string;
}

const replaceAt = (values: readonly JsonValue[], index: number, next: JsonValue): readonly JsonValue[] =>
  values.map((value, at) => (at === index ? next : value));

const moveAt = (values: readonly JsonValue[], index: number, delta: number): readonly JsonValue[] => {
  const target = index + delta;
  const moved = values[index];
  if (moved === undefined || target < 0 || target >= values.length) {
    return values;
  }
  const next = [...values];
  next.splice(index, 1);
  next.splice(target, 0, moved);
  return next;
};

export const RepeatField = ({
  label,
  fields,
  value,
  onChange,
  renderControl,
  path,
}: RepeatFieldProps): ReactElement => {
  const { t } = useTranslator(common, editor);
  return (
    <fieldset className="block-field repeat-field">
      <legend>{label}</legend>
      <ul className="repeat-rows">
        {value.map((entry, index) => {
          const record = isJsonObject(entry) ? entry : {};
          return (
            <li className="repeat-row" key={String(index)}>
              {fields.map((field) =>
                renderControl({
                  field,
                  value: record[field.key],
                  onChange: (next) => {
                    onChange(replaceAt(value, index, { ...record, [field.key]: next }));
                  },
                  path: `${path}.${String(index)}.${field.key}`,
                  record,
                  siblings: fields,
                  onPatch: (values) => {
                    onChange(replaceAt(value, index, { ...record, ...values }));
                  },
                }),
              )}
              <p className="repeat-actions">
                <button
                  type="button"
                  disabled={index === 0}
                  aria-label={t('repeat.moveUp', { position: index + 1 })}
                  onClick={() => {
                    onChange(moveAt(value, index, -1));
                  }}
                >
                  {t('blocks.moveUpText')}
                </button>
                <button
                  type="button"
                  disabled={index === value.length - 1}
                  aria-label={t('repeat.moveDown', { position: index + 1 })}
                  onClick={() => {
                    onChange(moveAt(value, index, 1));
                  }}
                >
                  {t('blocks.moveDownText')}
                </button>
                <button
                  type="button"
                  aria-label={t('repeat.remove', { position: index + 1 })}
                  onClick={() => {
                    onChange(value.filter((_, at) => at !== index));
                  }}
                >
                  {t('action.remove')}
                </button>
              </p>
            </li>
          );
        })}
      </ul>
      <p>
        <button
          type="button"
          onClick={() => {
            onChange([...value, {}]);
          }}
        >
          {t('repeat.add')}
        </button>
      </p>
    </fieldset>
  );
};
