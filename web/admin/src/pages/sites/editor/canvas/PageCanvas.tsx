/**
 * The page canvas: the draft's blocks as cards, in the page's own layout, with
 * the gestures that reorder, resize, group and delete them.
 *
 * The canvas is a controlled view. It reads a `PageDraft` and a selection, and
 * every edit leaves as a `DraftAction` through `dispatch`. The shell owns the
 * reducer and the form beside the canvas; the canvas owns only the transient
 * gesture state below.
 */
import { Fragment, useEffect, useRef, useState } from 'react';
import type { DragEvent, ReactElement } from 'react';
import { useTranslator } from '../../../../i18n';
import type { JsonValue } from '../../../../api/types';
import type { TemplateChoice } from '../../templates';
import type { BlockDraft, BlockKey, DraftAction, PageDraft, ValidationIssue } from '../draft';
import { validate } from '../draft';
import { parseNodes } from '../groups';
import type { DraftNode, GroupNode } from '../groups';
import { emptySlots, gridColumns, sectionOf } from '../sections';
import type { SectionOf } from '../sections';
import { BlockPicker, useInsertChoices } from './BlockPicker';
import { CanvasCard } from './CanvasCard';
import { gridGap, pageColumns } from './grid';
import { flatIndex, intoGroup } from './move';
import { counted } from '../messages';
import { romkaState } from '../romekaHash';
import type { Selection } from '../shell/selection';
import { blockSummary, layoutLabel, sectionLabel } from './summary';
import { blockImage, imageKey, useThumbnails } from './thumbnails';
import { editor } from '../../../../i18n/namespaces/editor';
import { sites } from '../../../../i18n/namespaces/sites';
import './canvas.css';

export interface PageCanvasProps {
  readonly draft: PageDraft;
  /** The site's `o:site_settings`, which carry its saved templates. */
  readonly siteSettings: JsonValue | null;
  /** The shell's selection. A selection made elsewhere scrolls its card into view. */
  readonly selection: Selection | null;
  readonly onSelect: (key: BlockKey | null) => void;
  readonly dispatch: (action: DraftAction) => void;
}

interface InsertPointProps {
  readonly index: number;
  readonly registry: PageDraft['registry'];
  readonly templates: readonly TemplateChoice[];
  readonly open: boolean;
  readonly onOpen: () => void;
  readonly onClose: () => void;
  readonly onChoose: (layout: string, index: number) => void;
  readonly onChooseTemplate: (template: TemplateChoice, index: number) => void;
  readonly onDropIndex: (index: number) => void;
}

const InsertPoint = ({
  index,
  registry,
  templates,
  open,
  onOpen,
  onClose,
  onChoose,
  onChooseTemplate,
  onDropIndex,
}: InsertPointProps): ReactElement => {
  const { t } = useTranslator(sites, editor);
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
        <BlockPicker
          registry={registry}
          templates={templates}
          index={index}
          onChoose={onChoose}
          onChooseTemplate={onChooseTemplate}
          onClose={onClose}
        />
      ) : null}
    </li>
  );
};

export const PageCanvas = ({
  draft,
  siteSettings,
  selection,
  onSelect,
  dispatch,
}: PageCanvasProps): ReactElement => {
  const { t } = useTranslator(sites, editor);
  const selectedKey = selection?.key ?? null;
  const list = useRef<HTMLUListElement | null>(null);
  useEffect(() => {
    if (selection === null || selection.from === 'outline') {
      return;
    }
    for (const card of list.current?.querySelectorAll('[data-key]') ?? []) {
      if (card.getAttribute('data-key') === selection.key) {
        card.scrollIntoView({ block: 'nearest' });
      }
    }
  }, [selection]);
  const templates = useInsertChoices(siteSettings);
  const [pickerIndex, setPickerIndex] = useState<number | null>(null);
  const [draggingKey, setDraggingKey] = useState<BlockKey | null>(null);
  const [pickedKey, setPickedKey] = useState<BlockKey | null>(null);
  const [announcement, setAnnouncement] = useState('');

  const grid = draft.layout === 'grid';
  const columns = Math.max(1, pageColumns(draft.layoutData));
  const columnGap = gridGap(draft.layoutData, 'grid_column_gap');
  const rowGap = gridGap(draft.layoutData, 'grid_row_gap');
  const nodes = parseNodes(draft.blocks);
  const thumbnails = useThumbnails(draft.registry, draft.blocks);
  const imageUrl = (block: BlockDraft): string | null => {
    const ref = blockImage(draft.registry, block);
    return ref === null ? null : (thumbnails.get(imageKey(ref)) ?? null);
  };
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

  const dropInto = (group: BlockKey, before: BlockKey | null = null): void => {
    if (draggingKey === null || draggingKey === before) {
      return;
    }
    dispatch(intoGroup(draft.blocks, draggingKey, group, before));
    setDraggingKey(null);
  };

  const choose = (layout: string, index: number): void => {
    dispatch({ type: 'addBlock', layout, index });
    setPickerIndex(null);
  };

  const chooseTemplate = (template: TemplateChoice, index: number): void => {
    dispatch({ type: 'insertBlocks', index, blocks: template.blocks });
    setPickerIndex(null);
  };

  const insertPoint = (index: number): ReactElement => (
    <InsertPoint
      key={`insert-${String(index)}`}
      index={index}
      registry={draft.registry}
      templates={templates}
      open={pickerIndex === index}
      onOpen={() => {
        setPickerIndex(index);
      }}
      onClose={() => {
        setPickerIndex(null);
      }}
      onChoose={choose}
      onChooseTemplate={chooseTemplate}
      onDropIndex={dropAt}
    />
  );

  const renderCard = (block: BlockDraft): ReactElement => {
    const label = layoutLabel(draft.registry, block, t);
    return (
      <CanvasCard
        key={block.key}
        block={block}
        blocks={draft.blocks}
        label={label}
        summary={blockSummary(block, draft.title, t)}
        image={imageUrl(block)}
        issues={issuesByBlock.get(block.key) ?? []}
        mismatch={romkaState(draft.registry, block) === 'edited'}
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

  /** A drop target that puts the dragged block into `group`, before `before` or at the end. */
  const dropTarget = (group: BlockKey, before: BlockKey | null) => ({
    onDragOver: (event: DragEvent) => {
      event.preventDefault();
    },
    onDrop: (event: DragEvent) => {
      event.preventDefault();
      event.stopPropagation();
      dropInto(group, before);
    },
  });

  /** A group with a section layout: its blocks in the section's own columns, one slot each. */
  const renderSection = (node: GroupNode, section: SectionOf): ReactElement => {
    const empty = emptySlots(section.shape, node.members.length);
    const layoutName = sectionLabel(draft.registry, section.layout, t);
    return (
      <Fragment key={node.marker.key}>
        {insertPoint(flatIndex(draft.blocks, node.marker.key))}
        {renderCard(node.marker)}
        <li
          className="canvas-group canvas-section"
          data-section={section.layout}
          aria-label={t('canvas.section', { layout: layoutName })}
        >
          <span className="canvas-group-label">
            {layoutName} · {counted(t, 'blocks', node.members.length)}
          </span>
          <ul
            className={section.shape.cards ? 'canvas-slots canvas-slots-cards' : 'canvas-slots'}
            style={{ gridTemplateColumns: gridColumns(section.shape) }}
          >
            {node.members.map((member, index) => (
              <li
                key={member.key}
                className="canvas-slot"
                data-slot={index + 1}
                {...dropTarget(node.marker.key, member.key)}
              >
                <ul className="canvas-slot-items">
                  {insertPoint(flatIndex(draft.blocks, member.key))}
                  {renderCard(member)}
                </ul>
              </li>
            ))}
            {Array.from({ length: empty }, (_, index) => (
              <li
                key={`empty-${String(index)}`}
                className="canvas-slot canvas-slot-empty"
                data-slot={node.members.length + index + 1}
                data-drop-into={node.marker.key}
                {...dropTarget(node.marker.key, null)}
              >
                {t('canvas.slotEmpty', { slot: node.members.length + index + 1 })}
              </li>
            ))}
          </ul>
          {empty > 0 ? null : (
            <div
              className="canvas-drop-into"
              data-drop-into={node.marker.key}
              {...dropTarget(node.marker.key, null)}
            >
              <span className="canvas-visually-hidden">{t('canvas.groupDrop')}</span>
            </div>
          )}
        </li>
      </Fragment>
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
    const section = sectionOf(node.marker);
    if (section !== null) {
      return renderSection(node, section);
    }
    return (
      <Fragment key={node.marker.key}>
        {insertPoint(flatIndex(draft.blocks, node.marker.key))}
        {renderCard(node.marker)}
        <li className="canvas-group">
          <span className="canvas-group-label">
            {counted(t, 'groupBlocks', node.members.length)}
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
    <section className="canvas" aria-label={t('canvas.region')}>
      <div className="canvas-toolbar">
        <p className="canvas-count">{counted(t, 'blocks', draft.blocks.length)}</p>
      </div>
      <p className="canvas-live" role="status" aria-live="polite">
        {announcement}
      </p>
      {pageIssueCount === 0 ? null : (
        <p className="canvas-page-error" role="alert">
          {counted(t, 'pageIssues', pageIssueCount)}
        </p>
      )}
      <ul
        ref={list}
        className={grid ? 'canvas-list canvas-grid' : 'canvas-list'}
        style={gridStyle}
      >
        {nodes.map(renderNode)}
        {insertPoint(draft.blocks.length)}
      </ul>
    </section>
  );
};
