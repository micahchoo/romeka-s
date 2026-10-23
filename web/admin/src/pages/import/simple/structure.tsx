/**
 * Structure: how the archive becomes items and sets. The server's proposal
 * is a suggestion the person corrects. The tree (`structure-tree.tsx`)
 * shows each folder that becomes items, each folder left out and each list
 * no folder uses; this inspector, the question zoom, shows the selected
 * node's questions, each with the suggested answer (✓) and its rival, and
 * with the counts each answer makes before any click.
 *
 * Each answer the person picks is one click (`structure-edits.ts`): it turns
 * on at once, its counts follow, and the changes strip under the tree lists
 * every click with what it changed and a way to take it back.
 *
 * Under the questions, a folder's node opens what "Your list" did for it
 * (`structure-list.tsx`, a lazy chunk): where each column goes, pair by
 * hand, fill in a list yourself and every file.
 *
 * A folder can also be written by hand: "Mark an example" opens the hand
 * edit (`structure-hand.tsx`, a lazy chunk of its own). The folder is then
 * "written by you" until the person goes back to the suggestion, and the
 * clicks on it wait meanwhile.
 */
import { Suspense, lazy, useState } from 'react';
import type { ReactElement } from 'react';
import type { SeriesNode } from '../../../api/import-proposal';
import { folderName, keptWords, seriesItems } from './structure-lines';
import { ImportQuestion, Notes, Row, useScreen } from './structure-parts';
import { ItemsQuestion, ListQuestions, SetQuestion, SplitOffer } from './structure-questions';
import { StructureTree } from './structure-tree';
import type { InspectorProps } from './structure-tree';

const OtherInspector = lazy(() => import('./structure-nodes').then((module) => ({ default: module.OtherInspector })));
const HandPanel = lazy(() => import('./structure-hand').then((module) => ({ default: module.HandPanel })));
const OwnInspector = lazy(() => import('./structure-hand').then((module) => ({ default: module.OwnInspector })));
const ListHomes = lazy(() => import('./structure-list').then((module) => ({ default: module.ListHomes })));

const SeriesInspector = ({ node, name, nodeKey }: { readonly node: SeriesNode; readonly name: string; readonly nodeKey: string }): ReactElement => {
  const { clicks, editable, words } = useScreen();
  const { t, number } = words;
  const [marking, setMarking] = useState(false);
  if (marking) {
    return (
      <div className="guided-inspect">
        <h4>{name}</h4>
        <Suspense fallback={<p className="import-slow">{t('shell.loading')}</p>}>
          <HandPanel
            clicks={clicks}
            editable={editable}
            level={node.level}
            scope={node.scope.scope}
            onDone={() => {
              setMarking(false);
            }}
          />
        </Suspense>
      </div>
    );
  }
  const kept = node.kept ?? [];
  return (
    <div className="guided-inspect">
      <h4>{name}</h4>
      <dl className="guided-kv">
        <ImportQuestion scope={node.scope} items={seriesItems(node)} />
        <ItemsQuestion
          node={node}
          onMark={() => {
            setMarking(true);
          }}
        />
        <ListQuestions node={node} />
        {kept.length === 0 ? null : (
          <Row label={t('structure.kept.title')}>
            <Notes lines={kept.map((one) => keptWords(one, t, number))} />
          </Row>
        )}
        <SetQuestion node={node} name={name} />
        {node.splits.map((split) => (
          <Row key={JSON.stringify(split.value)} label={t('structure.q.split')}>
            <SplitOffer split={split} />
          </Row>
        ))}
      </dl>
      <Suspense fallback={null}>
        <ListHomes nodeKey={nodeKey} level={node.level} folder={node.scope.scope.folder} />
      </Suspense>
    </div>
  );
};

const Inspector = ({ node, where, files, folders }: InspectorProps): ReactElement => {
  const { words } = useScreen();
  switch (node.kind) {
    case 'series':
      return <SeriesInspector node={node.node} name={folderName(node.node.scope.scope.folder, where)} nodeKey={node.key} />;
    case 'own':
      return (
        <Suspense fallback={<p className="import-slow">{words.t('shell.loading')}</p>}>
          <OwnInspector node={node.node} name={folderName(node.node.scope.scope.folder, where)} nodeKey={node.key} />
        </Suspense>
      );
    default:
      return (
        <Suspense fallback={<p className="import-slow">{words.t('shell.loading')}</p>}>
          <OtherInspector node={node} where={where} files={files} folders={folders} />
        </Suspense>
      );
  }
};

/** Simple's Structure: the question zoom of every node. */
export const Structure = (): ReactElement => <StructureTree Inspector={Inspector} />;

export default Structure;
