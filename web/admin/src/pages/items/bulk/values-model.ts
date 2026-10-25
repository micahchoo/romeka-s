/**
 * The Values section's form: one value change being written, before it joins the change. The
 * form keeps every input, so switching what to do and back loses nothing; `opOf` reads the one
 * change the inputs name, or nothing while they are incomplete.
 */
import type { ValueOp } from '../../../api/bulk-values';
import { brandId } from '../../../api/parse';
import type { PropertyId } from '../../../api/types';

export const ACTIONS = ['add', 'replace', 'clear', 'visibility'] as const;
export type ValueAction = (typeof ACTIONS)[number];

export const KINDS = ['literal', 'uri', 'item'] as const;
export type ValueKind = (typeof KINDS)[number];

export interface Composer {
  readonly property: PropertyId | null;
  readonly action: ValueAction;
  readonly kind: ValueKind;
  readonly text: string;
  readonly lang: string;
  readonly uri: string;
  readonly label: string;
  readonly item: string;
  /** Who sees an added value, or the visibility the field's values take. */
  readonly isPublic: boolean;
  readonly from: string;
  readonly to: string;
}

export const NEW_COMPOSER: Composer = {
  property: null,
  action: 'add',
  kind: 'literal',
  text: '',
  lang: '',
  uri: '',
  label: '',
  item: '',
  isPublic: true,
  from: '',
  to: '',
};

const filled = (text: string): string | null => (text.trim() === '' ? null : text.trim());

/** A web address names its scheme, as `Uri::uriIsValid` needs; the server checks the rest. */
const isAddress = (text: string): boolean => /^[a-z][a-z0-9+.-]*:\S+$/i.test(text);

const itemOf = (text: string): number | null => {
  const id = Number(text.trim());
  return /^\d+$/.test(text.trim()) && Number.isSafeInteger(id) && id > 0 ? id : null;
};

/** The change the inputs name, or `null` while one is missing or does not read. */
export const opOf = (composer: Composer): ValueOp | null => {
  const { property, isPublic } = composer;
  if (property === null) {
    return null;
  }
  switch (composer.action) {
    case 'clear':
      return { op: 'clear', property };
    case 'visibility':
      return { op: 'visibility', property, isPublic };
    case 'replace': {
      const to = filled(composer.to) === null ? null : composer.to;
      return composer.from === '' || to === null || composer.from === to
        ? null
        : { op: 'replace', property, from: composer.from, to };
    }
    case 'add':
      switch (composer.kind) {
        case 'literal':
          return filled(composer.text) === null
            ? null
            : { op: 'add', property, isPublic, value: { type: 'literal', text: composer.text, lang: filled(composer.lang) } };
        case 'uri': {
          const uri = filled(composer.uri);
          return uri === null || !isAddress(uri)
            ? null
            : { op: 'add', property, isPublic, value: { type: 'uri', uri, label: filled(composer.label) } };
        }
        case 'item': {
          const item = itemOf(composer.item);
          return item === null
            ? null
            : { op: 'add', property, isPublic, value: { type: 'item', item: brandId<'ItemId'>(item) } };
        }
      }
  }
};
