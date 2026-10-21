/**
 * The parsers of the import plan (`import-plan.ts`). Each refuses a member the
 * server's serde types refuse, so what it returns serialises back to the JSON
 * it read.
 */
import { STARTER_IDS } from './starters';
import type { StarterId } from './starters';
import {
  asCount,
  asInteger,
  asJson,
  asPositive,
  dictionary,
  list,
  member,
  nullable,
  oneOf,
  strict,
  wordOr,
} from './import-read';
import {
  ACTIONS,
  DELIMITERS,
  ENCODINGS,
  FILE_FACTS,
  FLAG_KINDS,
  FLAG_POLICIES,
  NAMED_SLOTS,
  RECORD_KINDS,
  TAG_IDS,
  isTerm,
} from './import-plan';
import type {
  ActionPlan,
  AnomalyAnswers,
  Change,
  ColumnPlan,
  ColumnRef,
  Decision,
  DecisionValue,
  Defaults,
  Encoding,
  EntryRef,
  FileEntryRef,
  FileRef,
  FlagKind,
  FlagPolicy,
  ImportPlan,
  Join,
  MatchRule,
  NewType,
  Quote,
  ReadOptions,
  RecordKind,
  RowRef,
  Scope,
  SheetPlan,
  SlotName,
  SlotRule,
  SlotRules,
  Slots,
  SourcePlan,
  SourceRef,
  Term,
  ValueType,
  Visibility,
} from './import-plan';
import { ParseError, asBoolean, asNumber, asRecord, asString } from './parse';
import type { Parser, Path } from './types';

export const asTerm = (value: unknown, path: Path): Term => {
  const text = asString(value, path);
  if (!isTerm(text)) {
    throw new ParseError(path, 'a term, prefix:local');
  }
  return text;
};


export const asSlotName = (value: unknown, path: Path): SlotName => {
  const text = asString(value, path);
  const named = NAMED_SLOTS.find((slot) => slot === text);
  if (named !== undefined) {
    return named;
  }
  if (!isTerm(text)) {
    throw new ParseError(path, 'a slot name or a term');
  }
  return text;
};

/** The slots that hold one value: more than one flags `many-values`. */

const text = (object: Record<string, unknown>, key: string, path: Path): string => member(object, key, path, asString);
const flag = (object: Record<string, unknown>, key: string, path: Path): boolean => member(object, key, path, asBoolean);
const maybeText = nullable(asString);

const asVisibility = oneOf<Visibility>(['public', 'private']);
const asRecordKind = oneOf(RECORD_KINDS);
const asFlagKind = oneOf(FLAG_KINDS);
const asFlagPolicy = oneOf(FLAG_POLICIES);

const parseColumnRef: Parser<ColumnRef> = (raw, path = []) => {
  const object = strict(raw, path, ['index', 'header']);
  return { index: member(object, 'index', path, asCount), header: text(object, 'header', path) };
};

export const parseValueType: Parser<ValueType> = (raw, path = []) => {
  if (typeof raw === 'string') {
    return oneOf<'literal' | 'uri'>(['literal', 'uri'])(raw, path);
  }
  const object = asRecord(raw, path);
  if ('resource' in object) {
    const only = strict(raw, path, ['resource']);
    return { resource: member(only, 'resource', path, oneOf<'identifier' | 'internalId'>(['identifier', 'internalId'])) };
  }
  const only = strict(raw, path, ['dataType']);
  return { dataType: text(only, 'dataType', path) };
};

export const parseSource: Parser<SourcePlan> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const kind = member(object, 'kind', path, oneOf<'folder' | 'staging'>(['folder', 'staging']));
  if (kind === 'staging') {
    const only = strict(raw, path, ['kind', 'id']);
    return { id: text(only, 'id', path), kind };
  }
  const only = strict(raw, path, ['kind', 'id', 'root', 'path']);
  return { id: text(only, 'id', path), kind, root: text(only, 'root', path), path: text(only, 'path', path) };
};

export const parseReadOptions: Parser<ReadOptions> = (raw, path = []) => {
  const object = strict(raw, path, ['encoding', 'delimiter', 'quote', 'headerLine', 'trim'], ['sheetName']);
  // The server leaves the member out for the first sheet and reads null alike.
  const named = object['sheetName'];
  const sheetName = named === undefined || named === null ? undefined : asString(named, [...path, 'sheetName']);
  return {
    encoding: member(object, 'encoding', path, wordOr<'auto', Encoding>(['auto'], oneOf(ENCODINGS))),
    delimiter: member(object, 'delimiter', path, oneOf(DELIMITERS)),
    quote: member(object, 'quote', path, oneOf<Quote>(['"', "'", 'none'])),
    headerLine: member(object, 'headerLine', path, asPositive),
    trim: flag(object, 'trim', path),
    ...(sheetName === undefined ? {} : { sheetName }),
  };
};

const parseColumnPlan: Parser<ColumnPlan> = (raw, path = []) => {
  const object = strict(raw, path, ['column', 'options']);
  const options = strict(object['options'], [...path, 'options'], ['split', 'type', 'lang', 'private']);
  const at = [...path, 'options'];
  return {
    column: member(object, 'column', path, parseColumnRef),
    options: {
      split: member(options, 'split', at, maybeText),
      type: member(options, 'type', at, parseValueType),
      lang: member(options, 'lang', at, maybeText),
      private: flag(options, 'private', at),
    },
  };
};

const parseJoin: Parser<Join> = (raw, path = []) => {
  const object = strict(raw, path, ['on', 'column', 'normalize']);
  const at = [...path, 'normalize'];
  const normalize = strict(object['normalize'], at, ['caseFold', 'leadingZeros', 'separators']);
  return {
    on: member(object, 'on', path, oneOf<'identifier' | 'fileName'>(['identifier', 'fileName'])),
    column: member(object, 'column', path, parseColumnRef),
    normalize: {
      caseFold: flag(normalize, 'caseFold', at),
      leadingZeros: flag(normalize, 'leadingZeros', at),
      separators: flag(normalize, 'separators', at),
    },
  };
};

const parseRowKind = wordOr<RecordKind, { readonly column: ColumnRef }>(RECORD_KINDS, (raw, path) => {
  const object = strict(raw, path, ['column']);
  return { column: member(object, 'column', path, parseColumnRef) };
});

const parseSheet: Parser<SheetPlan> = (raw, path = []) => {
  const object = strict(raw, path, ['id', 'source', 'file', 'hash', 'read', 'rowKind', 'join', 'itemKey', 'columns']);
  return {
    id: text(object, 'id', path),
    source: text(object, 'source', path),
    file: text(object, 'file', path),
    hash: member(object, 'hash', path, maybeText),
    read: member(object, 'read', path, parseReadOptions),
    rowKind: member(object, 'rowKind', path, parseRowKind),
    join: member(object, 'join', path, nullable(parseJoin)),
    itemKey: member(object, 'itemKey', path, nullable(parseColumnRef)),
    columns: member(object, 'columns', path, list(parseColumnPlan)),
  };
};

const parseSourceRef: Parser<SourceRef> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const from = member(object, 'from', path, oneOf(['group', 'folder', 'column', 'tag', 'file', 'constant'] as const));
  switch (from) {
    case 'group': {
      const only = strict(raw, path, ['from', 'name']);
      return { from, name: text(only, 'name', path) };
    }
    case 'folder': {
      const only = strict(raw, path, ['from', 'level']);
      return { from, level: member(only, 'level', path, wordOr<'path', number>(['path'], asPositive)) };
    }
    case 'column': {
      const only = strict(raw, path, ['from', 'sheet', 'column']);
      return { from, sheet: text(only, 'sheet', path), column: member(only, 'column', path, parseColumnRef) };
    }
    case 'tag': {
      const only = strict(raw, path, ['from', 'tag', 'files']);
      const files = wordOr<'first' | 'all', { readonly part: string }>(['first', 'all'], (value, at) => ({
        part: text(strict(value, at, ['part']), 'part', at),
      }));
      return {
        from,
        tag: member(only, 'tag', path, oneOf(TAG_IDS)),
        files: member(only, 'files', path, files),
      };
    }
    case 'file': {
      const only = strict(raw, path, ['from', 'fact']);
      return { from, fact: member(only, 'fact', path, oneOf(FILE_FACTS)) };
    }
    case 'constant': {
      const only = strict(raw, path, ['from', 'value']);
      return { from, value: text(only, 'value', path) };
    }
  }
};

const parseSlotRule: Parser<SlotRule> = (raw, path = []) => {
  const object = strict(raw, path, ['sources', 'combine'], ['map']);
  const rule = {
    sources: member(object, 'sources', path, list(parseSourceRef)),
    combine: member(object, 'combine', path, oneOf<'first' | 'all'>(['first', 'all'])),
  };
  return object['map'] === undefined ? rule : { ...rule, map: member(object, 'map', path, dictionary(asString)) };
};

const parseSlotRules = (raw: unknown, path: Path): SlotRules => {
  const object = asRecord(raw, path);
  const rules: Record<string, SlotRule> = {};
  for (const [name, rule] of Object.entries(object)) {
    asSlotName(name, [...path, name]);
    rules[name] = parseSlotRule(rule, [...path, name]);
  }
  return rules;
};

const parseSlots: Parser<Slots> = (raw, path = []) => {
  const object = strict(raw, path, ['item', 'media', 'itemSet']);
  return {
    item: member(object, 'item', path, parseSlotRules),
    media: member(object, 'media', path, parseSlotRules),
    itemSet: member(object, 'itemSet', path, parseSlotRules),
  };
};

const parseScope: (value: unknown, path: Path) => Scope = wordOr<'importSets' | 'all', { readonly sets: readonly number[] }>(
  ['importSets', 'all'],
  (raw, path) => ({ sets: member(strict(raw, path, ['sets']), 'sets', path, list(asInteger)) }),
);

const parseMatchRule: Parser<MatchRule> = (raw, path = []) => {
  const object = strict(raw, path, ['key', 'scope']);
  const key = member(object, 'key', path, (value, at): Term | 'internalId' =>
    value === 'internalId' ? value : asTerm(value, at),
  );
  return { key, scope: member(object, 'scope', path, parseScope) };
};

const parseActionPlan: Parser<ActionPlan> = (raw, path = []) => {
  const object = strict(raw, path, ['action', 'match', 'unmatched']);
  const at = [...path, 'match'];
  const match = strict(object['match'], at, ['item', 'media', 'itemSet']);
  return {
    action: member(object, 'action', path, oneOf(ACTIONS)),
    match: {
      item: member(match, 'item', at, parseMatchRule),
      media: member(match, 'media', at, parseMatchRule),
      itemSet: member(match, 'itemSet', at, parseMatchRule),
    },
    unmatched: member(object, 'unmatched', path, oneOf<'create' | 'leave'>(['create', 'leave'])),
  };
};

const parseDefaults: Parser<Defaults> = (raw, path = []) => {
  const object = strict(raw, path, ['visibility', 'owner', 'sites', 'template']);
  return {
    visibility: member(object, 'visibility', path, asVisibility),
    owner: member(object, 'owner', path, wordOr<'runner', number>(['runner'], asPositive)),
    sites: member(object, 'sites', path, list(asPositive)),
    template: member(object, 'template', path, nullable(asPositive)),
  };
};

const parseNewType: Parser<NewType> = (raw, path = []) => {
  const object = strict(raw, path, ['label', 'from', 'class']);
  const from = wordOr<'mapping', { readonly starter: StarterId }>(['mapping'], (value, at) => ({
    starter: member(strict(value, at, ['starter']), 'starter', at, oneOf(STARTER_IDS)),
  }));
  return {
    label: text(object, 'label', path),
    from: member(object, 'from', path, from),
    class: member(object, 'class', path, nullable(asTerm)),
  };
};

const parseFileRef: Parser<FileRef> = (raw, path = []) => {
  const object = strict(raw, path, ['source', 'path']);
  return { source: text(object, 'source', path), path: text(object, 'path', path) };
};

const parseRowRef: Parser<RowRef> = (raw, path = []) => {
  const object = strict(raw, path, ['sheet', 'key']);
  return { sheet: text(object, 'sheet', path), key: text(object, 'key', path) };
};

export const parseFileEntry: Parser<FileEntryRef> = (raw, path = []) => {
  const object = strict(raw, path, ['file']);
  return { file: member(object, 'file', path, parseFileRef) };
};

export const parseEntryRef: Parser<EntryRef> = (raw, path = []) => {
  const object = asRecord(raw, path);
  if ('row' in object) {
    return { row: member(strict(raw, path, ['row']), 'row', path, parseRowRef) };
  }
  return parseFileEntry(raw, path);
};

export { parseFileRef, parseRowRef };
const parseMoveTo = wordOr<'own' | 'leave', { readonly with: FileEntryRef }>(['own', 'leave'], (raw, path) => ({
  with: member(strict(raw, path, ['with']), 'with', path, parseFileEntry),
}));

const parseDecisionValue: Parser<DecisionValue> = (raw, path = []) => {
  const object = strict(raw, path, ['type', 'text', 'lang', 'label']);
  return {
    type: member(object, 'type', path, parseValueType),
    text: text(object, 'text', path),
    lang: member(object, 'lang', path, maybeText),
    label: member(object, 'label', path, maybeText),
  };
};

const parseChange: Parser<Change> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const kind = member(object, 'kind', path, oneOf(['move', 'leave', 'set', 'match', 'flag'] as const));
  switch (kind) {
    case 'move': {
      const only = strict(raw, path, ['kind', 'to']);
      return { kind, to: member(only, 'to', path, parseMoveTo) };
    }
    case 'leave':
      strict(raw, path, ['kind']);
      return { kind };
    case 'set': {
      const only = strict(raw, path, ['kind', 'target', 'slot', 'values']);
      return {
        kind,
        target: member(only, 'target', path, asRecordKind),
        slot: member(only, 'slot', path, asSlotName),
        values: member(only, 'values', path, list(parseDecisionValue)),
      };
    }
    case 'match': {
      const only = strict(raw, path, ['kind', 'target', 'resource']);
      return {
        kind,
        target: member(only, 'target', path, asRecordKind),
        resource: member(only, 'resource', path, nullable(asPositive)),
      };
    }
    case 'flag': {
      const only = strict(raw, path, ['kind', 'flag', 'slot', 'policy']);
      return {
        kind,
        flag: member(only, 'flag', path, asFlagKind),
        slot: member(only, 'slot', path, nullable(asSlotName)),
        policy: member(only, 'policy', path, asFlagPolicy),
      };
    }
  }
};

const parseDecision: Parser<Decision> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const kind = member(object, 'kind', path, oneOf(['move', 'leave', 'set', 'match', 'flag', 'many', 'pair'] as const));
  switch (kind) {
    case 'move': {
      const only = strict(raw, path, ['kind', 'file', 'to']);
      return { kind, file: member(only, 'file', path, parseFileEntry), to: member(only, 'to', path, parseMoveTo) };
    }
    case 'leave': {
      const only = strict(raw, path, ['kind', 'entry']);
      return { kind, entry: member(only, 'entry', path, parseEntryRef) };
    }
    case 'set': {
      const only = strict(raw, path, ['kind', 'entry', 'target', 'slot', 'values']);
      return {
        kind,
        entry: member(only, 'entry', path, parseEntryRef),
        target: member(only, 'target', path, asRecordKind),
        slot: member(only, 'slot', path, asSlotName),
        values: member(only, 'values', path, list(parseDecisionValue)),
      };
    }
    case 'match': {
      const only = strict(raw, path, ['kind', 'entry', 'target', 'resource']);
      return {
        kind,
        entry: member(only, 'entry', path, parseEntryRef),
        target: member(only, 'target', path, asRecordKind),
        resource: member(only, 'resource', path, nullable(asPositive)),
      };
    }
    case 'flag': {
      const only = strict(raw, path, ['kind', 'entry', 'flag', 'slot', 'policy']);
      return {
        kind,
        entry: member(only, 'entry', path, parseEntryRef),
        flag: member(only, 'flag', path, asFlagKind),
        slot: member(only, 'slot', path, nullable(asSlotName)),
        policy: member(only, 'policy', path, asFlagPolicy),
      };
    }
    case 'many': {
      const only = strict(raw, path, ['kind', 'entries', 'change']);
      return {
        kind,
        entries: member(only, 'entries', path, list(parseEntryRef)),
        change: member(only, 'change', path, parseChange),
      };
    }
    case 'pair': {
      const only = strict(raw, path, ['kind', 'row', 'file']);
      return { kind, row: member(only, 'row', path, parseRowRef), file: member(only, 'file', path, parseFileRef) };
    }
  }
};

const parseFlags = (raw: unknown, path: Path): ImportPlan['flags'] => {
  const flags: { [kind in FlagKind]?: FlagPolicy } = {};
  for (const [name, policy] of Object.entries(asRecord(raw, path))) {
    flags[asFlagKind(name, [...path, name])] = asFlagPolicy(policy, [...path, name]);
  }
  return flags;
};

const parseAnomalyAnswers: Parser<AnomalyAnswers> = (raw, path = []) => {
  const object = strict(raw, path, ['identical', 'unmatchedFiles', 'itemsWithoutRow']);
  return {
    identical: member(object, 'identical', path, oneOf<'keepOne' | 'keepAll'>(['keepOne', 'keepAll'])),
    unmatchedFiles: member(object, 'unmatchedFiles', path, oneOf<'leave' | 'own'>(['leave', 'own'])),
    itemsWithoutRow: member(object, 'itemsWithoutRow', path, oneOf<'keep' | 'leave'>(['keep', 'leave'])),
  };
};

const parseExample = dictionary(asJson);

export const parseSourcesOf = list(parseSource);

/**
 * A v1 plan carries no `version`; the server answers it 409 `v1Plan`, and the
 * client refuses it here instead of showing a plan it cannot edit.
 */
export const parseImportPlan: Parser<ImportPlan> = (raw, path = []) => {
  const object = strict(
    raw,
    path,
    ['version', 'sources', 'grouping', 'example', 'sheets', 'slots', 'action', 'defaults', 'flags', 'decisions', 'newTypes', 'newTerms'],
    ['anomalies', 'simple'],
  );
  if (member(object, 'version', path, asNumber) !== 2) {
    throw new ParseError([...path, 'version'], 'version 2');
  }
  const grouping = strict(object['grouping'], [...path, 'grouping'], ['pattern', 'caseFold']);
  const plan: ImportPlan = {
    version: 2,
    sources: member(object, 'sources', path, list(parseSource)),
    grouping: {
      pattern: member(grouping, 'pattern', [...path, 'grouping'], maybeText),
      caseFold: flag(grouping, 'caseFold', [...path, 'grouping']),
    },
    example: member(object, 'example', path, nullable(parseExample)),
    sheets: member(object, 'sheets', path, list(parseSheet)),
    slots: member(object, 'slots', path, parseSlots),
    action: member(object, 'action', path, parseActionPlan),
    defaults: member(object, 'defaults', path, parseDefaults),
    flags: member(object, 'flags', path, parseFlags),
    decisions: member(object, 'decisions', path, list(parseDecision)),
    newTypes: member(object, 'newTypes', path, list(parseNewType)),
    newTerms: member(object, 'newTerms', path, list((value, at) => ({ label: text(strict(value, at, ['label']), 'label', at) }))),
  };
  const withAnomalies: ImportPlan =
    object['anomalies'] === undefined ? plan : { ...plan, anomalies: member(object, 'anomalies', path, parseAnomalyAnswers) };
  return object['simple'] === undefined ? withAnomalies : { ...withAnomalies, simple: member(object, 'simple', path, parseExample) };
};

