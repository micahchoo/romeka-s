/**
 * The page canvas: the draft's blocks as cards, in the page's own layout, with
 * the gestures that reorder, resize, group and delete them.
 *
 * The canvas is a controlled view. It reads a `PageDraft` and a selection, and
 * every edit leaves as a `DraftAction` through `dispatch`. The shell owns the
 * reducer and the form beside the canvas; the canvas owns only the transient
 * gesture state below.
 */
import { Fragment, useState } from 'react';
import type { KeyboardEvent as ReactKeyboardEvent, ReactElement } from 'react';
import { useTranslator } from '../../../../i18n';
import type { BlockDraft, BlockKey, DraftAction, PageDraft, ValidationIssue } from '../draft';
import { validate } from '../draft';
import { parseNodes } from '../groups';
import type { DraftNode } from '../groups';
import { BlockPicker } from './BlockPicker';
import { CanvasCard } from './CanvasCard';
import { gridGap, pageColumns } from './grid';
import { flatIndex, intoGroup } from './move';
import { blockSummary, layoutLabel } from './summary';
import './canvas.css';

export interface PageCanvasProps {
  readonly draft: PageDraft;
  readonly selectedKey: BlockKey | null;
  readonly onSelect: (key: BlockKey | null) => void;
  readonly dispatch: (action: DraftAction) => void;
}

interface InsertPointProps {
  readonly index: number;
  readonly registry: PageDraft['registry'];
  readonly open: boolean;
  readonly onOpen: () => void;
  readonly onClose: () => void;
  readonly onChoose: (layout: string, index: number) => void;
  readonly onDropIndex: (index: number) => void;
}

const InsertPoint = ({
  index,
  registry,
  open,
  onOpen,
  onClose,
  onChoose,
  onDropIndex,
}: InsertPointProps): ReactElement => {
  const { t } = useTranslator();
  return (
    <li
      className="canvas-insert"
      onDragOver={(event) => {
        event.preventDefault();
      }}
      onDrop={(event) => {
        event.preventDefault();
        onDropIndex(index);
      }}
    >
      <button
        type="button"
        className="canvas-insert-button"
        aria-expanded={open}
        aria-label={t('canvas.insertAt', { position: index + 1 })}
        onClick={onOpen}
      >
        {t('canvas.insert')}
      </button>
      {open ? (
        <BlockPicker registry={registry} index={index} onChoose={onChoose} onClose={onClose} />
      ) : null}
    </li>
  );
};

export const PageCanvas = ({
  draft,
  selectedKey,
  onSelect,
  dispatch,
}: PageCanvasProps): ReactElement => {
  const { t } = useTranslator();
  const [pickerIndex, setPickerIndex] = useState<number | null>(null);
  const [draggingKey, setDraggingKey] = useState<BlockKey | null>(null);
  const [pickedKey, setPickedKey] = useState<BlockKey | null>(null);
  const [announcement, setAnnouncement] = useState('');

  const grid = draft.layout === 'grid';
  const columns = Math.max(1, pageColumns(draft.layoutData));
  const rawColumns = pageColumns(draft.layoutData);
  const columnGap = gridGap(draft.layoutData, 'grid_column_gap');
  const rowGap = gridGap(draft.layoutData, 'grid_row_gap');
  const nodes = parseNodes(draft.blocks);
  const issues = validate(draft);

  const issuesByBlock = new Map<BlockKey, readonly ValidationIssue[]>();
  for (const issue of issues) {
    if (issue.blockKey !== null) {
      issuesByBlock.set(issue.blockKey, [...(issuesByBlock.get(issue.blockKey) ?? []), issue]);
    }
  }
  const pageIssueCount = issues.filter((issue) => issue.blockKey === null).length;

  const dropAt = (index: number): void => {
    if (draggingKey === null) {
      return;
    }
    dispatch({ type: 'moveBlock', blockKey: draggingKey, toIndex: index });
    setDraggingKey(null);
  };

  const dropInto = (group: BlockKey): void => {
    if (draggingKey === null) {
      return;
    }
    dispatch(intoGroup(draft.blocks, draggingKey, group));
    setDraggingKey(null);
  };

  const choose = (layout: string, index: number): void => {
    dispatch({ type: 'addBlock', layout, index });
    setPickerIndex(null);
  };

  const setPageNumber = (key: string, text: string): void => {
    const value = Number.parseInt(text, 10);
    dispatch({ type: 'setPageLayoutData', key, value: Number.isNaN(value) ? 0 : value });
  };

  const shortcut = (event: ReactKeyboardEvent<HTMLElement>): void => {
    if (!(event.metaKey || event.ctrlKey) || (event.key !== 'z' && event.key !== 'Z')) {
      return;
    }
    const target = event.target;
    if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
      return;
    }
    event.preventDefault();
    dispatch({ type: event.shiftKey ? 'redo' : 'undo' });
  };

  const insertPoint = (index: number): ReactElement => (
    <InsertPoint
      key={`insert-${String(index)}`}
      index={index}
      registry={draft.registry}
      open={pickerIndex === index}
      onOpen={() => {
        setPickerIndex(index);
      }}
      onClose={() => {
        setPickerIndex(null);
      }}
      onChoose={choose}
      onDropIndex={dropAt}
    />
  );

  const renderCard = (block: BlockDraft): ReactElement => {
    const label = layoutLabel(draft.registry, block.layout, t);
    return (
      <CanvasCard
        key={block.key}
        block={block}
        blocks={draft.blocks}
        label={label}
        summary={blockSummary(block, draft.title, t)}
        issues={issuesByBlock.get(block.key) ?? []}
        selected={selectedKey === block.key}
        picked={pickedKey === block.key}
        grid={grid}
        columns={columns}
        onSelect={onSelect}
        onPickUp={(key) => {
          setPickedKey(key);
        }}
        onDrop={() => {
          setPickedKey(null);
        }}
        onCancel={() => {
          setPickedKey(null);
        }}
        dispatch={dispatch}
        onDragStart={(key) => {
          setDraggingKey(key);
        }}
        onDragEnd={() => {
          setDraggingKey(null);
        }}
        onAnnounce={setAnnouncement}
      />
    );
  };

  const renderNode = (node: DraftNode): ReactElement => {
    if (node.kind === 'block') {
      return (
        <Fragment key={node.block.key}>
          {insertPoint(flatIndex(draft.blocks, node.block.key))}
          {renderCard(node.block)}
        </Fragment>
      );
    }
    return (
      <Fragment key={node.marker.key}>
        {insertPoint(flatIndex(draft.blocks, node.marker.key))}
        {renderCard(node.marker)}
        <li className="canvas-group">
          <span className="canvas-group-label">
            {t('canvas.group', { count: node.members.length })}
          </span>
          <ul className="canvas-group-members">
            {node.members.map((member) => (
              <Fragment key={member.key}>
                {insertPoint(flatIndex(draft.blocks, member.key))}
                {renderCard(member)}
              </Fragment>
            ))}
            <li
              className="canvas-drop-into"
              data-drop-into={node.marker.key}
              onDragOver={(event) => {
                event.preventDefault();
              }}
              onDrop={(event) => {
                event.preventDefault();
                dropInto(node.marker.key);
              }}
            >
              <span className="canvas-visually-hidden">{t('canvas.groupDrop')}</span>
            </li>
          </ul>
        </li>
      </Fragment>
    );
  };

  const gridStyle = grid
    ? {
        gridTemplateColumns: `repeat(${String(columns)}, minmax(0, 1fr))`,
        columnGap: `${String(columnGap)}px`,
        rowGap: `${String(rowGap)}px`,
      }
    : undefined;

  return (
    <section className="canvas" aria-label={t('canvas.region')} onKeyDown={shortcut}>
      <div className="canvas-toolbar">
        <fieldset className="canvas-layout">
          <legend>{t('canvas.pageLayout')}</legend>
          <label>
            <input
              type="radio"
              name="canvas-page-layout"
              checked={!grid}
              onChange={() => {
                dispatch({ type: 'setPageLayout', layout: 'normal' });
              }}
            />
            {t('canvas.normal')}
          </label>
          <label>
            <input
              type="radio"
              name="canvas-page-layout"
              checked={grid}
              onChange={() => {
                dispatch({ type: 'setPageLayout', layout: 'grid' });
              }}
            />
            {t('canvas.grid')}
          </label>
        </fieldset>
        {grid ? (
          <div className="canvas-grid-fields">
            <label>
              {t('pageLayoutData.grid_columns')}
              <input
                type="number"
                min={1}
                value={rawColumns > 0 ? String(rawColumns) : ''}
                onChange={(event) => {
                  setPageNumber('grid_columns', event.target.value);
                }}
              />
            </label>
            <label>
              {t('pageLayoutData.grid_column_gap')}
              <input
                type="number"
                min={0}
                value={String(columnGap)}
                onChange={(event) => {
                  setPageNumber('grid_column_gap', event.target.value);
                }}
              />
            </label>
            <label>
              {t('pageLayoutData.grid_row_gap')}
              <input
                type="number"
                min={0}
                value={String(rowGap)}
                onChange={(event) => {
                  setPageNumber('grid_row_gap', event.target.value);
                }}
              />
            </label>
          </div>
        ) : null}
        <div className="canvas-history">
          <button
            type="button"
            disabled={draft.past.length === 0}
            onClick={() => {
              dispatch({ type: 'undo' });
            }}
          >
            {t('canvas.undo')}
          </button>
          <button
            type="button"
            disabled={draft.future.length === 0}
            onClick={() => {
              dispatch({ type: 'redo' });
            }}
          >
            {t('canvas.redo')}
          </button>
        </div>
      </div>
      <p className="canvas-live" role="status" aria-live="polite">
        {announcement}
      </p>
      {pageIssueCount === 0 ? null : (
        <p className="canvas-page-error" role="alert">
          {t('canvas.pageIssues', { count: pageIssueCount })}
        </p>
      )}
      {nodes.length === 0 ? <p className="canvas-empty">{t('canvas.empty')}</p> : null}
      <ul
        className={grid ? 'canvas-list canvas-grid' : 'canvas-list'}
        style={gridStyle}
      >
        {nodes.map(renderNode)}
        {insertPoint(draft.blocks.length)}
      </ul>
    </section>
  );
};
