/**
 * The inspectors of the Structure screen's nodes that are not a folder of
 * items: the archive, the sets over every item, a set level (counted, or
 * kept around a folder written by hand), a folder not imported and a list
 * no folder uses. They are a lazy chunk of their own:
 * the screen opens on a folder, and loads these when the person selects one.
 */
import { useState } from 'react';
import type { ReactElement } from 'react';
import type { OutsideFolder, Pick, SeriesNode, SetCounts } from '../../../api/import-proposal';
import type { SetFrom } from '../../../api/import-plan';
import type { ListCandidate } from '../../../api/import-sections';
import { QUESTIONS } from './structure-edits';
import { folderName, keptSetName, kindsWords, listsIn, setName, setNote, setsWords } from './structure-lines';
import type { TreeNode } from './structure-lines';
import { Offer, Row, useScreen } from './structure-parts';

const ArchiveInspector = ({ where, files }: { readonly where: string; readonly files: number }): ReactElement => {
  const { proposal, words } = useScreen();
  const { t, number } = words;
  const lists = listsIn(proposal);
  const series = proposal.nodes.filter((node) => node.kind === 'series' || node.kind === 'own').length;
  return (
    <div className="guided-inspect">
      <h4>{where}</h4>
      <dl className="guided-kv">
        <Row label={t('structure.root.files')}>{t('structure.root.filesIn', { files: number(files), where })}</Row>
        <Row label={t('structure.root.folders')}>{t('structure.root.foldersLine', { in: number(series), out: number(proposal.outside.length) })}</Row>
        <Row label={t('structure.root.lists')}>{t('structure.root.listsLine', { used: number(lists.used), unused: number(lists.unused) })}</Row>
      </dl>
    </div>
  );
};

const EveryInspector = ({ offers }: { readonly offers: readonly Pick<SetFrom, SetCounts>[] }): ReactElement => {
  const { words } = useScreen();
  const { t, number } = words;
  return (
    <div className="guided-inspect">
      <h4>{t('structure.every.title')}</h4>
      <p>{t('structure.every.note')}</p>
      <dl className="guided-kv">
        {offers.map((offer) => (
          <Row key={JSON.stringify(offer.value)} label={setName(offer.counts, t, number)}>
            <Offer text={setNote(offer.value, offer.counts, t, number)} action={t('structure.every.add')} asked={QUESTIONS.set(offer.value)} edit={{ edit: 'addSet', from: offer.value }} />
          </Row>
        ))}
      </dl>
    </div>
  );
};

/** A folder not imported. One the person left out comes back by taking the edit back; one with nothing to show takes a `keep`. */
const OutsideInspector = ({ folder, name }: { readonly folder: OutsideFolder; readonly name: string }): ReactElement => {
  const { clicks, editable, words } = useScreen();
  const { t, number } = words;
  const asked = QUESTIONS.import(folder.scope);
  return (
    <div className="guided-inspect">
      <h4>{name}</h4>
      <p>{folder.reason === 'noMedia' ? t('structure.out.why') : t('structure.out.whyEdit')}</p>
      <p>{t('structure.import.files', { files: number(folder.files), kinds: kindsWords(folder.kinds, t, number) })}</p>
      <button
        type="button"
        disabled={!editable}
        onClick={() => {
          clicks.answer(asked, 'keep', folder.reason === 'edit' ? null : { edit: 'keep', scope: folder.scope });
        }}
      >
        {t('structure.out.keep')}
      </button>
      {clicks.failed === asked ? <p role="alert">{t('structure.click.failed')}</p> : null}
    </div>
  );
};

/** A list no folder uses: it may describe any folder that is imported. */
const UnusedInspector = ({ list, folders, where }: { readonly list: ListCandidate; readonly folders: readonly SeriesNode[]; readonly where: string }): ReactElement => {
  const { clicks, editable, words } = useScreen();
  const { t, number } = words;
  const [folder, setFolder] = useState(0);
  const paired = list.best?.paired ?? 0;
  const why =
    list.read !== 'ok'
      ? t('structure.unused.error', { error: list.read.error })
      : paired === 0
        ? t('structure.unused.noRow')
        : t('structure.unused.rows', { n: number(paired), of: number(list.rows) });
  const scope = folders[folder]?.scope.scope;
  return (
    <div className="guided-inspect">
      <h4 className="guided-file">{list.file}</h4>
      <p>{why}</p>
      {list.read !== 'ok' || scope === undefined ? null : (
        <div className="structure-offer">
          <label>
            {t('structure.unused.folder')}{' '}
            <select
              value={folder}
              disabled={!editable}
              onChange={(event) => {
                setFolder(Number(event.target.value));
              }}
            >
              {folders.map((one, at) => (
                <option key={one.level} value={at}>
                  {folderName(one.scope.scope.folder, where)}
                </option>
              ))}
            </select>
          </label>
          <button
            type="button"
            disabled={!editable}
            onClick={() => {
              clicks.answer(QUESTIONS.list(scope), 'use', { edit: 'list', scope, list: { source: list.source, file: list.file } });
            }}
          >
            {t('structure.unused.use')}
          </button>
          {clicks.failed === QUESTIONS.list(scope) ? <p role="alert">{t('structure.click.failed')}</p> : null}
        </div>
      )}
    </div>
  );
};

/** The inspector of a node that is not a folder of items. */
export const OtherInspector = ({
  node,
  where,
  files,
  folders,
}: {
  readonly node: Exclude<TreeNode, { readonly kind: 'series' | 'own' }>;
  readonly where: string;
  readonly files: number;
  readonly folders: readonly SeriesNode[];
}): ReactElement => {
  const { words } = useScreen();
  switch (node.kind) {
    case 'archive':
      return <ArchiveInspector where={where} files={files} />;
    case 'every':
      return <EveryInspector offers={node.offers} />;
    case 'set':
      return (
        <div className="guided-inspect">
          <h4>{setName(node.node.counts, words.t, words.number)}</h4>
          <p>{setNote(node.node.from, node.node.counts, words.t, words.number)}</p>
        </div>
      );
    case 'kept':
      return (
        <div className="guided-inspect">
          <h4>{keptSetName(node.node.set, words.t)}</h4>
          <p>{words.t('structure.keptSet.note')}</p>
          <p>{node.node.counts === null ? words.t('structure.keptSet.uncounted') : setsWords(node.node.counts, words.t, words.number)}</p>
        </div>
      );
    case 'outside':
      return <OutsideInspector folder={node.folder} name={folderName(node.folder.scope.folder, where)} />;
    case 'unused':
      return <UnusedInspector list={node.list} folders={folders} where={where} />;
  }
};
