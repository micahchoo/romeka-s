/**
 * The parsers of the plan's tree (`structure`) and of the person's edits on a
 * proposal, and the plan parser built on them. They are apart from
 * `import-plan-read.ts`, whose small parsers the preview sections read, so
 * the import routes load them only with the first answer that carries a
 * plan (`import-detail-read.ts`).
 */
import { asCount, asInteger, dictionary, list, member, nullable, oneOf, strict, wordOr } from './import-read';
import { RECORD_KINDS, SET_NAMES } from './import-plan';
import type {
  Correction,
  CutEdit,
  Edit,
  FolderScope,
  ImportPlan,
  ItemJoin,
  Level,
  ListFile,
  PairRule,
  RowCell,
  RowPair,
  SeriesScope,
  SetFrom,
  SetSource,
  Structure,
} from './import-plan';
import {
  asJoinOn,
  parseActionPlan,
  parseColumnRef,
  parseDecision,
  parseDefaults,
  parseExample,
  parseFileRef,
  parseFlags,
  parseGrouping,
  parseNewTerm,
  parseNewType,
  parseNormalize,
  parseRowRef,
  parseSheet,
  parseSlots,
  parseSource,
  parseSourceRef,
  withAdminState,
} from './import-plan-read';
import { ParseError, asNumber, asRecord, asString } from './parse';
import type { Parser, Path } from './types';

const text = (object: Record<string, unknown>, key: string, path: Path): string => member(object, key, path, asString);
const maybeText = nullable(asString);
const asRecordKind = oneOf(RECORD_KINDS);

export const parseFolderScope: Parser<FolderScope> = (raw, path = []) => {
  const object = strict(raw, path, ['source', 'folder']);
  return { source: text(object, 'source', path), folder: text(object, 'folder', path) };
};

const parseSeriesScope: (value: unknown, path: Path) => SeriesScope = wordOr<'all', readonly FolderScope[]>(['all'], list(parseFolderScope));

/** Serde's untagged `SetSource`: the one member a shape has says which shape it is. */
export const parseSetSource: Parser<SetSource> = (raw, path = []) => {
  const object = asRecord(raw, path);
  if ('id' in object) {
    return { id: member(strict(raw, path, ['id']), 'id', path, asInteger) };
  }
  if ('title' in object) {
    const only = strict(raw, path, ['title', 'description']);
    return { title: text(only, 'title', path), description: member(only, 'description', path, maybeText) };
  }
  if ('each' in object) {
    const at = [...path, 'each'];
    const each = strict(strict(raw, path, ['each'])['each'], at, ['sources', 'combine', 'names'], ['map']);
    const rule = {
      sources: member(each, 'sources', at, list(parseSourceRef)),
      combine: member(each, 'combine', at, oneOf<'first' | 'all'>(['first', 'all'])),
    };
    const names = member(each, 'names', at, oneOf(SET_NAMES));
    return {
      each: each['map'] === undefined ? { ...rule, names } : { ...rule, map: member(each, 'map', at, dictionary(asString)), names },
    };
  }
  throw new ParseError(path, 'a set source: { id }, { title, description } or { each }');
};

const parseCorrection: Parser<Correction> = (raw, path = []) => {
  const kind = member(asRecord(raw, path), 'kind', path, oneOf(['join', 'pair', 'leave'] as const));
  switch (kind) {
    case 'join': {
      const only = strict(raw, path, ['kind', 'file', 'with']);
      return { kind, file: member(only, 'file', path, parseFileRef), with: member(only, 'with', path, parseFileRef) };
    }
    case 'pair': {
      const only = strict(raw, path, ['kind', 'row', 'file']);
      return { kind, row: member(only, 'row', path, parseRowRef), file: member(only, 'file', path, parseFileRef) };
    }
    case 'leave':
      return { kind, row: member(strict(raw, path, ['kind', 'row']), 'row', path, parseRowRef) };
  }
};

const parseLevel: Parser<Level> = (raw, path = []) => {
  const object = asRecord(raw, path);
  const kind = member(object, 'kind', path, oneOf(['set', 'series'] as const));
  if (kind === 'set') {
    const only = strict(raw, path, ['kind', 'id', 'set', 'levels']);
    return {
      kind,
      id: text(only, 'id', path),
      set: member(only, 'set', path, parseSetSource),
      levels: member(only, 'levels', path, list(parseLevel)),
    };
  }
  const only = strict(raw, path, ['kind', 'id', 'set', 'scope', 'grouping', 'example', 'lists', 'slots'], ['corrections']);
  const series = {
    kind,
    id: text(only, 'id', path),
    set: member(only, 'set', path, nullable(parseSetSource)),
    scope: member(only, 'scope', path, parseSeriesScope),
    grouping: member(only, 'grouping', path, parseGrouping),
    example: member(only, 'example', path, nullable(parseExample)),
    lists: member(only, 'lists', path, list(parseSheet)),
    slots: member(only, 'slots', path, parseSlots),
  };
  return only['corrections'] === undefined ? series : { ...series, corrections: member(only, 'corrections', path, list(parseCorrection)) };
};

export const parseStructure: Parser<Structure> = (raw, path = []) => {
  const structure = strict(raw, path, ['levels', 'freeSets'], ['outside']);
  const tree = {
    levels: member(structure, 'levels', path, list(parseLevel)),
    freeSets: member(structure, 'freeSets', path, list(parseSetSource)),
  };
  return structure['outside'] === undefined
    ? tree
    : { ...tree, outside: member(structure, 'outside', path, list(parseFolderScope)) };
};

export const parseListFile: Parser<ListFile> = (raw, path = []) => {
  const object = strict(raw, path, ['source', 'file']);
  return { source: text(object, 'source', path), file: text(object, 'file', path) };
};

export const parseSetFrom: Parser<SetFrom> = (raw, path = []) => {
  const from = member(asRecord(raw, path), 'from', path, oneOf(['folder', 'column', 'namePlace', 'folderLevel'] as const));
  switch (from) {
    case 'folder':
      return { from, scope: member(strict(raw, path, ['from', 'scope']), 'scope', path, parseFolderScope) };
    case 'column': {
      const only = strict(raw, path, ['from', 'list', 'column']);
      return { from, list: member(only, 'list', path, parseListFile), column: member(only, 'column', path, parseColumnRef) };
    }
    case 'namePlace': {
      const only = strict(raw, path, ['from', 'scope', 'place']);
      return { from, scope: member(only, 'scope', path, parseFolderScope), place: member(only, 'place', path, asCount) };
    }
    case 'folderLevel': {
      const only = strict(raw, path, ['from', 'scope', 'depth']);
      return { from, scope: member(only, 'scope', path, parseFolderScope), depth: member(only, 'depth', path, asCount) };
    }
  }
};

export const parsePairRule: Parser<PairRule> = (raw, path = []) => {
  const rule = member(asRecord(raw, path), 'rule', path, oneOf(['folds', 'column', 'lastPlaces'] as const));
  switch (rule) {
    case 'folds':
      return { rule, normalize: member(strict(raw, path, ['rule', 'normalize']), 'normalize', path, parseNormalize) };
    case 'column': {
      const only = strict(raw, path, ['rule', 'column', 'on']);
      return { rule, column: member(only, 'column', path, parseColumnRef), on: member(only, 'on', path, asJoinOn) };
    }
    case 'lastPlaces':
      return { rule, places: member(strict(raw, path, ['rule', 'places']), 'places', path, asCount) };
  }
};

const parseCutEdit: Parser<CutEdit> = (raw, path = []) => {
  const cut = member(asRecord(raw, path), 'cut', path, oneOf(['whole', 'places', 'folder'] as const));
  switch (cut) {
    case 'whole':
      strict(raw, path, ['cut']);
      return { cut };
    case 'places':
      return { cut, places: member(strict(raw, path, ['cut', 'places']), 'places', path, asCount) };
    case 'folder':
      return { cut, depth: member(strict(raw, path, ['cut', 'depth']), 'depth', path, asCount) };
  }
};

export const parseItemJoin: Parser<ItemJoin> = (raw, path = []) => {
  const only = strict(raw, path, ['item', 'with']);
  return { item: text(only, 'item', path), with: text(only, 'with', path) };
};

export const parseRowCell: Parser<RowCell> = (raw, path = []) => {
  const only = strict(raw, path, ['line', 'cell']);
  return { line: member(only, 'line', path, asCount), cell: text(only, 'cell', path) };
};

export const parseRowPair: Parser<RowPair> = (raw, path = []) => {
  const only = strict(raw, path, ['row', 'file']);
  return { row: member(only, 'row', path, parseRowCell), file: text(only, 'file', path) };
};

/** One edit of a plan's `edits` (`plan/edits.rs`). */
export const parseEdit: Parser<Edit> = (raw, path = []) => {
  const edit = member(
    asRecord(raw, path),
    'edit',
    path,
    oneOf(['leaveOut', 'keep', 'items', 'list', 'grain', 'addSet', 'dropSet', 'pair', 'title', 'existing', 'own'] as const),
  );
  switch (edit) {
    case 'leaveOut':
    case 'keep':
    case 'own':
      return { edit, scope: member(strict(raw, path, ['edit', 'scope']), 'scope', path, parseFolderScope) };
    case 'items': {
      const only = strict(raw, path, ['edit', 'scope', 'cut']);
      return { edit, scope: member(only, 'scope', path, parseFolderScope), cut: member(only, 'cut', path, parseCutEdit) };
    }
    case 'list': {
      const only = strict(raw, path, ['edit', 'scope', 'list']);
      return { edit, scope: member(only, 'scope', path, parseFolderScope), list: member(only, 'list', path, nullable(parseListFile)) };
    }
    case 'grain': {
      const only = strict(raw, path, ['edit', 'list', 'describes']);
      return { edit, list: member(only, 'list', path, parseListFile), describes: member(only, 'describes', path, asRecordKind) };
    }
    case 'addSet':
    case 'dropSet':
      return { edit, from: member(strict(raw, path, ['edit', 'from']), 'from', path, parseSetFrom) };
    case 'pair': {
      const only = strict(raw, path, ['edit', 'list', 'rule']);
      return { edit, list: member(only, 'list', path, parseListFile), rule: member(only, 'rule', path, parsePairRule) };
    }
    case 'title': {
      const only = strict(raw, path, ['edit', 'from', 'title'], ['description']);
      const titled = { edit, from: member(only, 'from', path, parseSetFrom), title: text(only, 'title', path) };
      const description = only['description'] === undefined ? null : member(only, 'description', path, maybeText);
      return description === null ? titled : { ...titled, description };
    }
    case 'existing': {
      const only = strict(raw, path, ['edit', 'from', 'id']);
      return { edit, from: member(only, 'from', path, parseSetFrom), id: member(only, 'id', path, asInteger) };
    }
  }
};

/**
 * The plan the server answers, always version 3 (a stored v2 plan comes
 * lifted). A v1 plan carries no `version`; the server answers it 409
 * `v1Plan`, and the client refuses it here instead of showing a plan it
 * cannot edit.
 *
 * Refuses what the server's serde types refuse: an unknown member, a wrong
 * `kind` or `version`, a value of the wrong type, a set source of two shapes.
 * It does not repeat the rules `ImportPlan::from_json` adds after serde
 * (`plan/check.rs`): unique level and sheet ids, scopes that do not overlap or
 * name a missing source, slot rules that read only their own series' lists or
 * name `item.sets`, and titles that are not empty. The server answers those.
 */
export const parseImportPlan: Parser<ImportPlan> = (raw, path = []) => {
  const object = strict(
    raw,
    path,
    ['version', 'sources', 'structure', 'action', 'defaults', 'flags', 'decisions', 'newTypes', 'newTerms'],
    ['anomalies', 'simple', 'edits'],
  );
  if (member(object, 'version', path, asNumber) !== 3) {
    throw new ParseError([...path, 'version'], 'version 3');
  }
  const plan: ImportPlan = {
    version: 3,
    sources: member(object, 'sources', path, list(parseSource)),
    structure: member(object, 'structure', path, parseStructure),
    action: member(object, 'action', path, parseActionPlan),
    defaults: member(object, 'defaults', path, parseDefaults),
    flags: member(object, 'flags', path, parseFlags),
    decisions: member(object, 'decisions', path, list(parseDecision)),
    newTypes: member(object, 'newTypes', path, list(parseNewType)),
    newTerms: member(object, 'newTerms', path, list(parseNewTerm)),
  };
  const withState = withAdminState(plan, object, path);
  return object['edits'] === undefined ? withState : { ...withState, edits: member(object, 'edits', path, list(parseEdit)) };
};
