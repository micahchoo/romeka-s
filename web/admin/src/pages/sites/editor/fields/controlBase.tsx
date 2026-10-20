/**
 * The props and helpers every field control shares, so a control module beside
 * `FieldControl` can declare one kind without importing the dispatcher.
 */
import type { ReactElement, ReactNode } from 'react';
import type { JsonValue, SiteId } from '../../../../api/types';
import type { BlockField } from '../../blockSchema';
import type { BlockKey, ValidationIssue } from '../draft';
import type { EditorOptions } from '../options';
import { Help } from '../Labelled';

export type FieldOf<Type extends BlockField['type']> = Extract<BlockField, { type: Type }>;

/** A stored scalar as the string a text control shows. */
export const asText = (value: JsonValue | undefined): string => {
  if (typeof value === 'string') {
    return value;
  }
  return typeof value === 'number' || typeof value === 'boolean' ? String(value) : '';
};

/** The props every control of one field shares. */
export interface ControlProps {
  readonly id: string;
  readonly value: JsonValue | undefined;
  readonly onChange: (value: JsonValue) => void;
  readonly error: ReactNode;
  readonly invalid: boolean;
  readonly siteId: SiteId;
  readonly issues: readonly ValidationIssue[];
  readonly blockKey: BlockKey | null;
  readonly idPrefix: string;
  readonly path: string;
  /** The values beside this one, which `link`, `sides` and `itemQuery` read. */
  readonly record: Readonly<Record<string, JsonValue>>;
  readonly onPatch: (values: Readonly<Record<string, JsonValue>>) => void;
  /** The editor's shared lists, so a control makes no request of its own. */
  readonly options?: EditorOptions;
}

/** A control that renders its own label, followed by the field's help line. */
export const WithHelp = ({
  id,
  help,
  children,
}: {
  readonly id: string;
  readonly help: string;
  readonly children: ReactNode;
}): ReactElement => (
  <div className="block-field-help">
    {children}
    <Help id={id} text={help} />
  </div>
);
