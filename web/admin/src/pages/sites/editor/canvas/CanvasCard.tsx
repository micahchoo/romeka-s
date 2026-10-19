/**
 * One block on the canvas: its layout label, a content summary, a validation
 * badge, a small action menu, and the gestures that edit it.
 *
 * The card never changes a block. Every gesture becomes a `DraftAction` built
 * by `move.ts` and handed to `dispatch`. Reordering works by drag (HTML5 DnD,
 * with the drop index owned by the parent) and by keyboard: Space picks a card
 * up, the arrows move it, Space drops it and Escape cancels. In grid mode
 * Shift+Left and Shift+Right resize the span, Left and Right set the column,
 * and the right edge is a drag handle.
 */
import { useState } from 'react';
import type {
  DragEvent,
  KeyboardEvent as ReactKeyboardEvent,
  PointerEvent as ReactPointerEvent,
  ReactElement,
} from 'react';
import { useTranslator } from '../../../../i18n';
import type { BlockDraft, BlockKey, DraftAction, ValidationIssue } from '../draft';
import { placementOf } from './grid';
import {
  flatIndex,
  groupAbove,
  groupKeyOf,
  intoGroup,
  moveByOne,
  movePosition,
  outOfGroup,
  resizeSpan,
} from './move';

/** The pixels a resize drag travels for one column when the card width is unknown. */
const RESIZE_STEP = 40;

export interface CanvasCardProps {
  readonly block: BlockDraft;
  readonly blocks: readonly BlockDraft[];
  readonly label: string;
  readonly summary: string;
  /** The thumbnail of the image the block names, `null` when it names none. */
  readonly image: string | null;
  readonly issues: readonly ValidationIssue[];
  /** Whether the carrier's stored html was edited outside Romeka. */
  readonly mismatch: boolean;
  readonly selected: boolean;
  readonly picked: boolean;
  readonly grid: boolean;
  readonly columns: number;
  readonly onSelect: (key: BlockKey | null) => void;
  readonly onPickUp: (key: BlockKey) => void;
  readonly onDrop: () => void;
  readonly onCancel: () => void;
  readonly dispatch: (action: DraftAction) => void;
  readonly onDragStart: (key: BlockKey) => void;
  readonly onDragEnd: () => void;
  readonly onAnnounce: (message: string) => void;
}

export const CanvasCard = ({
  block,
  blocks,
  label,
  summary,
  image,
  issues,
  mismatch,
  selected,
  picked,
  grid,
  columns,
  onSelect,
  onPickUp,
  onDrop,
  onCancel,
  dispatch,
  onDragStart,
  onDragEnd,
  onAnnounce,
}: CanvasCardProps): ReactElement => {
  const { t } = useTranslator();
  const [menuOpen, setMenuOpen] = useState(false);
  const position = flatIndex(blocks, block.key) + 1;
  const placement = placementOf(block, columns);
  const group = groupKeyOf(blocks, block.key);
  const above = group === null ? groupAbove(blocks, block.key) : null;

  const name =
    issues.length === 0
      ? t('canvas.card', { position, label })
      : t('canvas.cardIssues', { position, label, count: issues.length });

  const remove = (): void => {
    setMenuOpen(false);
    dispatch({ type: 'removeBlock', blockKey: block.key });
    onAnnounce(t('canvas.deleted', { label }));
  };

  const moveOut = (): void => {
    setMenuOpen(false);
    dispatch(outOfGroup(blocks, block.key));
    onAnnounce(t('canvas.moved', { position, label }));
  };

  const moveInto = (): void => {
    if (above === null) {
      return;
    }
    setMenuOpen(false);
    dispatch(intoGroup(blocks, block.key, above));
    onAnnounce(t('canvas.moved', { position, label }));
  };

  const handleKey = (event: ReactKeyboardEvent<HTMLElement>, handle: boolean): void => {
    if (grid && (event.key === 'ArrowLeft' || event.key === 'ArrowRight')) {
      event.preventDefault();
      const delta: -1 | 1 = event.key === 'ArrowLeft' ? -1 : 1;
      if (event.shiftKey) {
        const change = resizeSpan(block, columns, delta);
        if (change !== null) {
          dispatch(change.action);
          onAnnounce(t('canvas.span', { position, span: change.span }));
        }
        return;
      }
      if (handle || picked) {
        const change = movePosition(block, columns, delta);
        if (change !== null) {
          dispatch(change.action);
          onAnnounce(t('canvas.column', { position, column: change.position }));
        }
      }
      return;
    }
    if (handle) {
      return;
    }
    if (event.key === ' ') {
      event.preventDefault();
      if (picked) {
        onDrop();
        onAnnounce(t('canvas.dropped', { label }));
      } else {
        onPickUp(block.key);
        onAnnounce(t('canvas.picked', { label }));
      }
      return;
    }
    if (event.key === 'Escape' && picked) {
      event.preventDefault();
      onCancel();
      onAnnounce(t('canvas.cancelled'));
      return;
    }
    if ((event.key === 'Delete' || event.key === 'Backspace') && !picked) {
      event.preventDefault();
      remove();
      return;
    }
    if ((event.key === 'ArrowUp' || event.key === 'ArrowDown') && picked) {
      event.preventDefault();
      const delta: -1 | 1 = event.key === 'ArrowUp' ? -1 : 1;
      const action = moveByOne(blocks, block.key, delta);
      if (action !== null) {
        dispatch(action);
        onAnnounce(t('canvas.moved', { position: position + delta, label }));
      }
    }
  };

  const handleResizeDown = (event: ReactPointerEvent<HTMLButtonElement>): void => {
    const startX = event.clientX;
    const startSpan = placement.span;
    const width = event.currentTarget.parentElement?.offsetWidth ?? 0;
    const step = width > 0 ? width / columns : RESIZE_STEP;
    let last = startSpan;
    const move = (moveEvent: PointerEvent): void => {
      const next = Math.min(
        columns,
        Math.max(1, startSpan + Math.round((moveEvent.clientX - startX) / step)),
      );
      if (next === last) {
        return;
      }
      last = next;
      dispatch({
        type: 'setLayoutData',
        blockKey: block.key,
        key: 'grid_column_span',
        value: String(next),
      });
    };
    const up = (): void => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      onAnnounce(t('canvas.span', { position, span: last }));
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  };

  const style = grid
    ? {
        gridColumnStart: placement.position === 'auto' ? 'auto' : String(placement.position),
        gridColumnEnd: `span ${String(placement.span)}`,
      }
    : undefined;

  return (
    <li
      className="canvas-card"
      style={style}
      data-key={block.key}
      data-selected={selected ? 'true' : undefined}
      data-picked={picked ? 'true' : undefined}
      aria-label={name}
      aria-current={selected ? 'true' : undefined}
      tabIndex={0}
      draggable
      onDragStart={(event: DragEvent<HTMLLIElement>) => {
        event.dataTransfer.setData('text/plain', block.key);
        event.dataTransfer.effectAllowed = 'move';
        onDragStart(block.key);
      }}
      onDragEnd={() => {
        onDragEnd();
      }}
      onClick={() => {
        onSelect(block.key);
      }}
      onFocus={() => {
        onSelect(block.key);
      }}
      onKeyDown={(event) => {
        if (event.target === event.currentTarget) {
          handleKey(event, false);
        }
      }}
    >
      <div className="canvas-card-head">
        {image === null ? null : <img className="canvas-card-thumb" src={image} alt="" />}
        <span className="canvas-card-label">{label}</span>
        <span className="canvas-card-summary">{summary}</span>
        {issues.length === 0 ? null : (
          <span className="canvas-badge">{t('canvas.issues', { count: issues.length })}</span>
        )}
        {mismatch ? (
          <>
            <span className="canvas-badge canvas-badge-edited">{t('canvas.romkaMismatch')}</span>
            <button
              type="button"
              className="canvas-romka-restore"
              onClick={() => {
                dispatch({ type: 'useRomkaVersion', blockKey: block.key });
                onAnnounce(t('canvas.romkaRestored', { label }));
              }}
            >
              {t('canvas.romkaUse')}
            </button>
          </>
        ) : null}
        <div className="canvas-menu">
          <button
            type="button"
            className="canvas-menu-button"
            aria-expanded={menuOpen}
            aria-label={t('canvas.menu', { position })}
            onClick={() => {
              setMenuOpen(!menuOpen);
            }}
          >
            {t('canvas.menuLabel')}
          </button>
          {menuOpen ? (
            <ul className="canvas-menu-list">
              <li>
                <button type="button" onClick={remove}>
                  {t('action.remove')}
                </button>
              </li>
              {group === null ? null : (
                <li>
                  <button type="button" onClick={moveOut}>
                    {t('canvas.outOfGroup')}
                  </button>
                </li>
              )}
              {above === null ? null : (
                <li>
                  <button type="button" onClick={moveInto}>
                    {t('canvas.intoGroup')}
                  </button>
                </li>
              )}
            </ul>
          ) : null}
        </div>
      </div>
      {grid ? (
        <button
          type="button"
          className="canvas-resize"
          aria-label={t('canvas.resizeLabel', { position })}
          onPointerDown={handleResizeDown}
          onKeyDown={(event) => {
            handleKey(event, true);
          }}
        />
      ) : null}
    </li>
  );
};
